use game_application::matchmaking::{
    HeartbeatSequence, MatchmakingError, MatchmakingRepository, Participation, RefusalReason,
    TicketId, TicketState,
};
use game_application::rating::{Pool, RatingState};
use game_application::{GameRepository, GameService, RequestId, Resign, UserId};
use game_persistence::SeaOrmStore;
use pg_test_support::TestDatabase;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};
use std::sync::Arc;

struct Fixture {
    _db: TestDatabase,
    store: Arc<SeaOrmStore>,
    raw: DatabaseConnection,
}
impl Fixture {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let store = Arc::new(SeaOrmStore::connect(db.url()).await.unwrap());
        let raw = Database::connect(db.url()).await.unwrap();
        Self {
            _db: db,
            store,
            raw,
        }
    }
    async fn count(&self, table: &str) -> i64 {
        self.raw
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                format!("SELECT count(*) AS n FROM {table}"),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "n")
            .unwrap()
    }
    async fn join(&self, user: &str, pool: Pool) -> TicketId {
        let id = TicketId::new();
        assert!(self.store.join(uid(user), id, pool).await.unwrap().created);
        id
    }
}
fn uid(s: &str) -> UserId {
    UserId::new(s).unwrap()
}
const POOL: Pool = Pool::TenMinute30;

#[tokio::test]
async fn durable_retry_refusal_and_cancel_before_join_survive_reconnect() {
    let f = Fixture::new().await;
    let first = f.join("one", POOL).await;
    let refused = TicketId::new();
    let r = f
        .store
        .join(uid("one"), refused, Pool::Byoyomi30)
        .await
        .unwrap();
    assert_eq!(
        r.ticket.state,
        TicketState::Refused(RefusalReason::AlreadyWaiting)
    );
    assert!(!r.created);
    f.store.cancel(&uid("one"), first).await.unwrap();
    assert_eq!(
        f.store
            .join(uid("one"), refused, Pool::Byoyomi30)
            .await
            .unwrap(),
        r
    );
    let early = TicketId::new();
    f.store.cancel(&uid("one"), early).await.unwrap();
    let restarted = SeaOrmStore::connect(f._db.url()).await.unwrap();
    for pool in Pool::ALL {
        assert_eq!(
            restarted
                .join(uid("one"), early, pool)
                .await
                .unwrap()
                .ticket
                .state,
            TicketState::Cancelled
        );
    }
    assert_eq!(
        restarted.ticket(&uid("other"), first).await,
        Err(MatchmakingError::NotFound)
    );
    let next = f.join("one", POOL).await;
    assert_eq!(
        f.store.join(uid("one"), next, Pool::Byoyomi30).await,
        Err(MatchmakingError::PoolConflict)
    );
    f.store.cancel(&uid("one"), first).await.unwrap();
    assert!(matches!(
        f.store.ticket(&uid("one"), next).await.unwrap().state,
        TicketState::Waiting(_)
    ));
}

#[tokio::test]
async fn cross_pool_joins_and_multiple_matchers_have_one_durable_winner() {
    let f = Fixture::new().await;
    let (a, b) = tokio::join!(
        f.store.join(uid("one"), TicketId::new(), POOL),
        f.store.join(uid("one"), TicketId::new(), Pool::Byoyomi30)
    );
    let results = [a.unwrap(), b.unwrap()];
    assert_eq!(results.iter().filter(|r| r.created).count(), 1);
    let pool = results
        .iter()
        .find(|r| r.created)
        .unwrap()
        .ticket
        .pool
        .unwrap();
    f.join("two", pool).await;
    let (a, b) = tokio::join!(f.store.tick(pool), f.store.tick(pool));
    assert_eq!(
        [a.unwrap(), b.unwrap()]
            .iter()
            .filter(|r| r.is_some())
            .count(),
        1
    );
    assert_eq!(f.count("games").await, 1);
    assert_eq!(f.count("rated_games").await, 1);
    assert_eq!(f.count("rating_slots").await, 2);
    assert!(
        matches!(f.store.current(&uid("one")).await.unwrap(),Participation::Active(t) if matches!(t.state,TicketState::Playing {..}))
    );
    let refusal = f
        .store
        .join(
            uid("one"),
            TicketId::new(),
            if pool == POOL { Pool::Byoyomi30 } else { POOL },
        )
        .await
        .unwrap();
    assert_eq!(
        refusal.ticket.state,
        TicketState::Refused(RefusalReason::AlreadyPlaying)
    );
}

#[tokio::test]
async fn pair_failure_rolls_back_and_terminal_failure_preserves_both_active_tickets() {
    let f = Fixture::new().await;
    let one = f.join("one", POOL).await;
    let two = f.join("two", POOL).await;
    f.raw.execute_unprepared("CREATE FUNCTION reject_slot() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected slot failure'; END $$; CREATE TRIGGER reject_slot BEFORE INSERT ON rating_slots FOR EACH ROW EXECUTE FUNCTION reject_slot()").await.unwrap();
    assert_eq!(f.store.tick(POOL).await, Err(MatchmakingError::Unavailable));
    for table in ["games", "rated_games", "rating_slots"] {
        assert_eq!(f.count(table).await, 0);
    }
    for (user, id) in [("one", one), ("two", two)] {
        assert!(matches!(
            f.store.ticket(&uid(user), id).await.unwrap().state,
            TicketState::Waiting(_)
        ));
    }
    f.raw
        .execute_unprepared("DROP TRIGGER reject_slot ON rating_slots")
        .await
        .unwrap();
    let game = f.store.tick(POOL).await.unwrap().unwrap();
    let identity = f.store.rated_game(&game).await.unwrap().unwrap();
    assert_eq!(identity.black_start, RatingState::Unrated);
    assert_eq!(identity.white_start, RatingState::Unrated);
    f.raw.execute_unprepared("CREATE TRIGGER reject_job BEFORE INSERT ON rating_jobs FOR EACH ROW EXECUTE FUNCTION reject_slot()").await.unwrap();
    let service = GameService::new(f.store.clone());
    let command = Resign {
        request_id: RequestId::new("finish").unwrap(),
        user_id: uid("one"),
        expected_seq: 0,
    };
    assert!(service.resign(&game, command.clone()).await.is_err());
    assert_eq!(f.store.version(&game).await.unwrap(), 0);
    for table in ["journal", "rating_jobs", "outbox_events"] {
        assert_eq!(f.count(table).await, 0);
    }
    for (user, id) in [("one", one), ("two", two)] {
        assert!(matches!(
            f.store.ticket(&uid(user), id).await.unwrap().state,
            TicketState::Playing { .. }
        ));
    }
    f.raw
        .execute_unprepared("DROP TRIGGER reject_job ON rating_jobs")
        .await
        .unwrap();
    let result = service.resign(&game, command.clone()).await.unwrap();
    assert_eq!(service.resign(&game, command).await.unwrap(), result);
    assert_eq!(
        f.store.current(&uid("one")).await.unwrap(),
        Participation::Idle
    );
    assert_eq!(f.count("rating_slots").await, 2);
    assert_eq!(
        f.store
            .join(uid("one"), TicketId::new(), POOL)
            .await
            .unwrap()
            .ticket
            .state,
        TicketState::Refused(RefusalReason::RatingPending)
    );
    let next = f.join("one", Pool::Byoyomi30).await;
    f.store.cancel(&uid("one"), one).await.unwrap();
    assert!(matches!(
        f.store.ticket(&uid("one"), next).await.unwrap().state,
        TicketState::Waiting(_)
    ));
}

#[tokio::test]
async fn heartbeat_retries_do_not_extend_and_expired_waiting_never_revives() {
    let f = Fixture::new().await;
    let id = f.join("one", POOL).await;
    let seq = HeartbeatSequence::new(1).unwrap();
    let result = f.store.heartbeat(&uid("one"), id, seq).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    assert_eq!(
        f.store.heartbeat(&uid("one"), id, seq).await.unwrap(),
        result
    );
    f.raw
        .execute_unprepared("UPDATE matchmaking_tickets SET enqueued_at=0, expires_at=1")
        .await
        .unwrap();
    assert_eq!(
        f.store
            .heartbeat(&uid("one"), id, HeartbeatSequence::new(2).unwrap())
            .await
            .unwrap()
            .state,
        TicketState::Expired
    );
    assert!(!f.store.join(uid("one"), id, POOL).await.unwrap().created);
    assert_eq!(
        f.store.current(&uid("one")).await.unwrap(),
        Participation::Idle
    );
}

#[tokio::test]
async fn expiry_after_rating_lock_wait_prevents_match() {
    let f = Fixture::new().await;
    let one = f.join("one", POOL).await;
    f.join("two", POOL).await;
    f.raw.execute_unprepared("UPDATE matchmaking_tickets SET expires_at=floor(extract(epoch FROM clock_timestamp())*1000)::bigint+150").await.unwrap();
    let tx = f.raw.begin().await.unwrap();
    tx.execute_unprepared("SELECT * FROM rating_states ORDER BY user_id FOR UPDATE")
        .await
        .unwrap();
    let store = f.store.clone();
    let pending = tokio::spawn(async move { store.tick(POOL).await });
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    tx.commit().await.unwrap();
    assert_eq!(pending.await.unwrap().unwrap(), None);
    assert_eq!(f.count("games").await, 0);
    assert_eq!(
        f.store.ticket(&uid("one"), one).await.unwrap().state,
        TicketState::Expired
    );
}
