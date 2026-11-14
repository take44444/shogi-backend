//! Low-level data fixtures for journal, reader, and rating-worker tests.
//! Production callers create games only through matchmaking.
use game_application::rating::{Pool, RatedGame, RatingState};
use game_application::{
    ApplicationError, GameId, GameRepository, GameService, Participants, Snapshot,
};
use sqlx::{Connection, PgConnection, Row};
use std::sync::Arc;

/// A game service with narrowly scoped SQL fixture construction.
#[derive(Clone)]
pub struct GameFixture {
    games: GameService,
    url: String,
}
impl GameFixture {
    /// Uses the supplied production repository for all game behavior.
    pub fn new(repository: Arc<dyn GameRepository>, url: &str) -> Self {
        Self {
            games: GameService::new(repository),
            url: url.into(),
        }
    }
    /// Seeds an unrated journal fixture and reads its initial snapshot.
    pub async fn seed(
        &self,
        id: GameId,
        players: Participants,
    ) -> Result<Snapshot, ApplicationError> {
        seed_game(&self.url, &id, &players).await?;
        self.games.snapshot(&id).await
    }
}
impl std::ops::Deref for GameFixture {
    type Target = GameService;
    fn deref(&self) -> &Self::Target {
        &self.games
    }
}
fn encoded(value: &str) -> String {
    value
        .replace('\u{1}', "\u{1}\u{2}")
        .replace('\0', "\u{1}\u{1}")
}
fn failure(error: sqlx::Error) -> ApplicationError {
    if error
        .as_database_error()
        .is_some_and(|e| e.is_unique_violation())
    {
        ApplicationError::AlreadyExists
    } else {
        ApplicationError::Unavailable
    }
}
async fn insert_game(
    c: &mut PgConnection,
    id: &GameId,
    p: &Participants,
) -> Result<(), ApplicationError> {
    sqlx::query("INSERT INTO games(id,black,white,version,rule,visibility,final_move_ply) VALUES($1,$2,$3,0,'standard','public',0)").bind(encoded(id.as_str())).bind(encoded(p.black().as_str())).bind(encoded(p.white().as_str())).execute(c).await.map_err(failure)?;
    Ok(())
}
/// Seeds an unrated record used by low-level game-history tests.
pub async fn seed_game(
    url: &str,
    id: &GameId,
    players: &Participants,
) -> Result<(), ApplicationError> {
    let mut c = PgConnection::connect(url).await.map_err(failure)?;
    let mut tx = c.begin().await.map_err(failure)?;
    insert_game(&mut tx, id, players).await?;
    for (user, side) in [(players.black(), 1), (players.white(), 2)] {
        sqlx::query("INSERT INTO matchmaking_tickets(user_id,ticket_id,pool,state,enqueued_at,expires_at,rating,completed,heartbeat_seq,game_id,side) VALUES($1,$2,'ten_minute_30','playing',0,1,NULL,0,0,$3,$4)").bind(encoded(user.as_str())).bind(uuid::Uuid::new_v4().to_string()).bind(encoded(id.as_str())).bind(side).execute(&mut *tx).await.map_err(failure)?;
    }
    tx.commit().await.map_err(failure)
}
/// Seeds a rated worker input with frozen states, reservations, and playing tickets.
/// Admission-policy tests use MatchmakingRepository instead.
pub async fn seed_rated(
    url: &str,
    id: &GameId,
    players: &Participants,
    pool: Pool,
) -> Result<(), ApplicationError> {
    let mut c = PgConnection::connect(url).await.map_err(failure)?;
    let mut tx = c.begin().await.map_err(failure)?;
    sqlx::query("SELECT pg_advisory_xact_lock(6001133723959083849)")
        .execute(&mut *tx)
        .await
        .map_err(failure)?;
    let mut users = [players.black(), players.white()];
    users.sort_by_key(|u| u.as_str());
    for user in users {
        sqlx::query("INSERT INTO rating_states(user_id,pool,rating,completed) VALUES($1,$2,NULL,0) ON CONFLICT DO NOTHING").bind(encoded(user.as_str())).bind(pool.as_str()).execute(&mut *tx).await.map_err(failure)?;
        sqlx::query("SELECT * FROM rating_states WHERE user_id=$1 AND pool=$2 FOR UPDATE")
            .bind(encoded(user.as_str()))
            .bind(pool.as_str())
            .fetch_one(&mut *tx)
            .await
            .map_err(failure)?;
        if sqlx::query("SELECT game_id FROM rating_slots WHERE user_id=$1 AND pool=$2")
            .bind(encoded(user.as_str()))
            .bind(pool.as_str())
            .fetch_optional(&mut *tx)
            .await
            .map_err(failure)?
            .is_some()
        {
            return Err(ApplicationError::RatingPoolBusy);
        }
    }
    insert_game(&mut tx, id, players).await?;
    sqlx::query("INSERT INTO rated_games(game_id,pool,black_start_rating,black_start_count,white_start_rating,white_start_count) SELECT $1,$2,b.rating,b.completed,w.rating,w.completed FROM rating_states b, rating_states w WHERE b.user_id=$3 AND w.user_id=$4 AND b.pool=$2 AND w.pool=$2").bind(encoded(id.as_str())).bind(pool.as_str()).bind(encoded(players.black().as_str())).bind(encoded(players.white().as_str())).execute(&mut *tx).await.map_err(failure)?;
    for (user, side) in [(players.black(), 1), (players.white(), 2)] {
        sqlx::query("INSERT INTO rating_slots(user_id,pool,game_id) VALUES($1,$2,$3)")
            .bind(encoded(user.as_str()))
            .bind(pool.as_str())
            .bind(encoded(id.as_str()))
            .execute(&mut *tx)
            .await
            .map_err(failure)?;
        sqlx::query("INSERT INTO matchmaking_tickets(user_id,ticket_id,pool,state,enqueued_at,expires_at,rating,completed,heartbeat_seq,game_id,side) SELECT $1,$2,$3,'playing',0,1,rating,completed,0,$4,$5 FROM rating_states WHERE user_id=$1 AND pool=$3").bind(encoded(user.as_str())).bind(uuid::Uuid::new_v4().to_string()).bind(pool.as_str()).bind(encoded(id.as_str())).bind(side).execute(&mut *tx).await.map_err(failure)?;
    }
    tx.commit().await.map_err(failure)
}
/// Reads the frozen identity of a rating fixture without a production creation API.
pub async fn rated_identity(url: &str, id: &GameId) -> Option<RatedGame> {
    let mut c = PgConnection::connect(url).await.unwrap();
    let r=sqlx::query("SELECT black,white,pool,black_start_rating,black_start_count,white_start_rating,white_start_count FROM rated_games JOIN games ON game_id=id WHERE game_id=$1").bind(encoded(id.as_str())).fetch_optional(&mut c).await.unwrap()?;
    let user = |key| {
        game_application::UserId::new(
            r.get::<String, _>(key)
                .replace("\u{1}\u{1}", "\0")
                .replace("\u{1}\u{2}", "\u{1}"),
        )
        .unwrap()
    };
    Some(RatedGame {
        game_id: id.clone(),
        participants: Participants::new(user("black"), user("white")).unwrap(),
        pool: Pool::parse(r.get::<&str, _>("pool")).unwrap(),
        black_start: RatingState::from_parts(
            r.get("black_start_rating"),
            r.get("black_start_count"),
        )
        .unwrap(),
        white_start: RatingState::from_parts(
            r.get("white_start_rating"),
            r.get("white_start_count"),
        )
        .unwrap(),
    })
}
