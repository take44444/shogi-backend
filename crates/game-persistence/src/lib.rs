//! PostgreSQL persistence for [`GameRepository`] through SeaORM.
//!
//! [`SeaOrmStore::connect`] initializes or validates the versioned schema before
//! returning. Startup uses a PostgreSQL advisory transaction lock. Each command
//! compares its expected version and writes metadata, its receipt, and frozen
//! outbox events in one transaction. Commit errors can be uncertain, so the
//! application reconciles them through a fresh load.
//!
//! Metadata and ordered receipts load through one left join, keeping each read
//! within one database snapshot. The application validates replay from a standard
//! game rather than restoring an untrusted position string.
#![warn(missing_docs)]

mod matchmaking;
mod matchmaking_schema;
mod outbox;
mod rating;
mod rating_schema;
mod schema;
pub use outbox::{OutboxOpenError, PostgresCursor, PostgresOutbox};
pub use rating::SeaOrmRatingJobs;

use async_trait::async_trait;
use game_application::{
    Acceptance, Action, ApplicationError as Error, ClockState, GameId, GameRecord, GameRepository,
    JournalEntry, LockedGameUpdate, Milliseconds, MutationRequest, Participants, Receipt,
    RequestId, StagedGameChange, TimeControl, Timestamp, UserId, ViewMetadata, ViewerGameRecord,
    Visibility,
};
use schema::a;
use sea_orm::sea_query::{Expr, ExprTrait, JoinType, Order, Query, Value};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbErr, QueryResult,
    TransactionTrait,
};
use shogi_domain::{FinishReason, GameResult, Player};

/// An initialized PostgreSQL connection pool implementing [`GameRepository`].
pub struct SeaOrmStore {
    db: DatabaseConnection,
}

impl SeaOrmStore {
    /// Opens PostgreSQL and initializes or validates its schema before returning.
    /// For a new database, creates the schema and version ledger atomically.
    /// Existing empty or incompatible ledgers and missing application tables fail
    /// closed. An advisory transaction lock serializes concurrent startup.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unavailable`] for connection or database errors and
    /// [`Error::Corrupt`] for an incompatible or incomplete schema.
    pub async fn connect(url: &str) -> Result<Self, Error> {
        let mut options = ConnectOptions::new(url);
        options
            .max_connections(5)
            .sqlx_logging(false)
            .map_sqlx_postgres_opts(|o| o.options([("search_path", "public")]));
        let db = Database::connect(options).await.map_err(storage)?;
        schema::initialize(&db).await.map_err(|error| match error {
            schema::SchemaError::Corrupt => Error::Corrupt,
            schema::SchemaError::Database(error) => storage(error),
        })?;
        Ok(Self { db })
    }

    /// Closes the shared connection pool through a reference.
    /// All services holding this store lose access to that pool; later operations
    /// can fail with [`Error::Unavailable`]. Coordinate shutdown with those users.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unavailable`] if the database pool fails to close.
    pub async fn close(&self) -> Result<(), Error> {
        self.db.close_by_ref().await.map_err(storage)
    }

    async fn load_view(
        db: &impl ConnectionTrait,
        id: &GameId,
        viewer: Option<&UserId>,
    ) -> Result<ViewerGameRecord, Error> {
        let mut query = Query::select();
        query.expr_as(database_time(), a("server_now"));
        for column in [
            "id",
            "black",
            "white",
            "version",
            "winner",
            "reason",
            "visibility",
            "initial_main_ms",
            "byoyomi_ms",
            "created_at",
            "clock_black",
            "clock_white",
            "clock_started",
            "clock_deadline",
            "clock_stopped",
            "byoyomi_black",
            "byoyomi_white",
        ] {
            query.expr_as(
                Expr::col((a("games"), a(column))),
                a(&format!("g_{column}")),
            );
        }
        for column in [
            "seq",
            "request_id",
            "user_id",
            "expected_seq",
            "action",
            "usi",
            "in_check",
            "winner",
            "reason",
            "evaluated_at",
            "loser",
            "deadline_at",
            "clock_black",
            "clock_white",
            "clock_started",
            "clock_deadline",
            "clock_stopped",
            "byoyomi_black",
            "byoyomi_white",
        ] {
            query.expr_as(Expr::col((a("journal"), a(column))), a(column));
        }
        query
            .from(a("games"))
            .join(
                JoinType::LeftJoin,
                a("journal"),
                Expr::col((a("games"), a("id"))).eq(Expr::col((a("journal"), a("game_id")))),
            )
            .and_where(Expr::col((a("games"), a("id"))).eq(encode_id(id.as_str())))
            .order_by((a("journal"), a("seq")), Order::Asc);
        let rows = db.query_all(&query).await.map_err(storage)?;
        let first = rows.first().ok_or(Error::NotFound)?;
        let participants = Participants::new(
            UserId::new(decode_id(
                &first.try_get::<String>("", "g_black").map_err(corrupt)?,
            )?)
            .map_err(corrupt)?,
            UserId::new(decode_id(
                &first.try_get::<String>("", "g_white").map_err(corrupt)?,
            )?)
            .map_err(corrupt)?,
        )
        .map_err(corrupt)?;
        let visibility = match first
            .try_get::<String>("", "g_visibility")
            .map_err(corrupt)?
            .as_str()
        {
            "public" => Visibility::Public,
            "private" => Visibility::Private,
            _ => return Err(Error::Corrupt),
        };
        let access = ViewMetadata {
            participants: participants.clone(),
            visibility,
        };
        if viewer.is_some_and(|viewer| !access.permits(viewer)) {
            return Err(Error::NotFound);
        }
        let control = TimeControl {
            initial_main_ms: Milliseconds::new(
                first.try_get("", "g_initial_main_ms").map_err(corrupt)?,
            )?,
            byoyomi_ms: Milliseconds::new(first.try_get("", "g_byoyomi_ms").map_err(corrupt)?)?,
        };
        let mut record = GameRecord {
            time_control: control,
            initial_clock: ClockState::start(
                control,
                Timestamp::new(first.try_get("", "g_created_at").map_err(corrupt)?)?,
            )?,
            clock: decode_clock(first, "g_")?,
            server_now_ms: Timestamp::new(first.try_get("", "server_now").map_err(corrupt)?)?,
            id: GameId::new(decode_id(
                &first.try_get::<String>("", "g_id").map_err(corrupt)?,
            )?)
            .map_err(corrupt)?,
            participants,
            version: unsigned(first.try_get("", "g_version").map_err(corrupt)?)?,
            result: result(first, "g_")?,
            journal: Vec::new(),
        };
        for row in rows {
            let seq: Option<i64> = row.try_get("", "seq").map_err(corrupt)?;
            let Some(seq) = seq else { continue };
            let evaluated_at = Timestamp::new(row.try_get("", "evaluated_at").map_err(corrupt)?)?;
            let acceptance = Acceptance {
                seq: unsigned(seq)?,
                in_check: row.try_get("", "in_check").map_err(corrupt)?,
                result: result(&row, "")?,
                clock: decode_clock(&row, "")?,
            };
            let expected_seq = unsigned(row.try_get("", "expected_seq").map_err(corrupt)?)?;
            let action_code = row.try_get::<i32>("", "action").map_err(corrupt)?;
            let usi: Option<String> = row.try_get("", "usi").map_err(corrupt)?;
            let entry = if action_code == 3 {
                if usi.is_some()
                    || row
                        .try_get::<Option<String>>("", "request_id")
                        .map_err(corrupt)?
                        .is_some()
                    || row
                        .try_get::<Option<String>>("", "user_id")
                        .map_err(corrupt)?
                        .is_some()
                {
                    return Err(Error::Corrupt);
                }
                JournalEntry::Timeout {
                    expected_seq,
                    loser: decode_player(row.try_get("", "loser").map_err(corrupt)?)?,
                    deadline_at: Timestamp::new(row.try_get("", "deadline_at").map_err(corrupt)?)?,
                    evaluated_at,
                    acceptance,
                }
            } else {
                if row
                    .try_get::<Option<i32>>("", "loser")
                    .map_err(corrupt)?
                    .is_some()
                    || row
                        .try_get::<Option<i64>>("", "deadline_at")
                        .map_err(corrupt)?
                        .is_some()
                {
                    return Err(Error::Corrupt);
                }
                let action = match (action_code, usi) {
                    (1, Some(usi)) => Action::Move(usi.parse().map_err(corrupt)?),
                    (2, None) => Action::Resign,
                    _ => return Err(Error::Corrupt),
                };
                Receipt {
                    evaluated_at,
                    request: MutationRequest {
                        request_id: RequestId::new(decode_id(
                            &row.try_get::<String>("", "request_id").map_err(corrupt)?,
                        )?)
                        .map_err(corrupt)?,
                        user_id: UserId::new(decode_id(
                            &row.try_get::<String>("", "user_id").map_err(corrupt)?,
                        )?)
                        .map_err(corrupt)?,
                        expected_seq,
                        action,
                    },
                    acceptance,
                }
                .into()
            };
            record.journal.push(entry);
        }
        Ok(ViewerGameRecord { access, record })
    }
}

fn storage(_: DbErr) -> Error {
    Error::Unavailable
}
fn corrupt(_: impl std::fmt::Debug) -> Error {
    Error::Corrupt
}
pub(crate) fn encode_id(value: &str) -> String {
    value
        .replace('\u{1}', "\u{1}\u{2}")
        .replace('\0', "\u{1}\u{1}")
}

pub(crate) fn decode_id(value: &str) -> Result<String, Error> {
    let mut decoded = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        decoded.push(match character {
            '\u{1}' => match chars.next() {
                Some('\u{2}') => '\u{1}',
                Some('\u{1}') => '\0',
                _ => return Err(Error::Corrupt),
            },
            other => other,
        });
    }
    Ok(decoded)
}

fn signed(seq: u64) -> Result<i64, Error> {
    i64::try_from(seq).map_err(|_| Error::SequenceExhausted)
}
fn unsigned(seq: i64) -> Result<u64, Error> {
    u64::try_from(seq).map_err(corrupt)
}
fn result_values(result: Option<GameResult>) -> (Value, Value) {
    match result {
        None => (Option::<i32>::None.into(), Option::<i32>::None.into()),
        Some(result) => (
            (match result.winner {
                Player::Black => 1,
                Player::White => 2,
            })
            .into(),
            (match result.reason {
                FinishReason::Checkmate => 1,
                FinishReason::Resignation => 2,
                FinishReason::Timeout => 3,
            })
            .into(),
        ),
    }
}
fn result(row: &QueryResult, prefix: &str) -> Result<Option<GameResult>, Error> {
    let winner: Option<i32> = row
        .try_get("", &format!("{prefix}winner"))
        .map_err(corrupt)?;
    let reason: Option<i32> = row
        .try_get("", &format!("{prefix}reason"))
        .map_err(corrupt)?;
    match (winner, reason) {
        (None, None) => Ok(None),
        (Some(winner), Some(reason)) => Ok(Some(GameResult {
            winner: match winner {
                1 => Player::Black,
                2 => Player::White,
                _ => return Err(Error::Corrupt),
            },
            reason: match reason {
                1 => FinishReason::Checkmate,
                2 => FinishReason::Resignation,
                3 => FinishReason::Timeout,
                _ => return Err(Error::Corrupt),
            },
        })),
        _ => Err(Error::Corrupt),
    }
}

async fn insert_game(
    tx: &sea_orm::DatabaseTransaction,
    id: &GameId,
    participants: &Participants,
    pool: rating_domain::Pool,
) -> Result<(), Error> {
    let control = TimeControl::for_pool(pool);
    let created_at = db_now(tx).await?;
    let clock = ClockState::start(control, created_at)?;
    let mut insert = Query::insert();
    insert.into_table(a("games"));
    insert.columns(
        [
            a("id"),
            a("black"),
            a("white"),
            a("version"),
            a("rule"),
            a("visibility"),
            a("final_move_ply"),
            a("initial_main_ms"),
            a("byoyomi_ms"),
            a("created_at"),
        ]
        .into_iter()
        .chain(clock_columns().map(a)),
    );
    insert.values_panic(
        vec![
            encode_id(id.as_str()).into(),
            encode_id(participants.black().as_str()).into(),
            encode_id(participants.white().as_str()).into(),
            0_i64.into(),
            "standard".into(),
            "public".into(),
            0_i64.into(),
            control.initial_main_ms.get().into(),
            control.byoyomi_ms.get().into(),
            created_at.get().into(),
        ]
        .into_iter()
        .chain(encode_clock(clock))
        .map(Expr::from),
    );
    let inserted = tx.execute(&insert).await;
    match inserted {
        Ok(_) => Ok(()),
        Err(error)
            if error
                .sql_err()
                .is_some_and(|e| matches!(e, sea_orm::SqlErr::UniqueConstraintViolation(_))) =>
        {
            Err(Error::AlreadyExists)
        }
        Err(error) => Err(storage(error)),
    }
}

#[async_trait]
impl GameRepository for SeaOrmStore {
    async fn load_game_for_viewer(
        &self,
        id: &GameId,
        viewer: &UserId,
    ) -> Result<ViewerGameRecord, Error> {
        Self::load_view(&self.db, id, Some(viewer)).await
    }

    async fn view_metadata(&self, id: &GameId, viewer: &UserId) -> Result<ViewMetadata, Error> {
        let row = self
            .db
            .query_one(
                Query::select()
                    .columns([a("black"), a("white"), a("visibility")])
                    .from(a("games"))
                    .and_where(Expr::col(a("id")).eq(encode_id(id.as_str()))),
            )
            .await
            .map_err(storage)?
            .ok_or(Error::NotFound)?;
        let participants = Participants::new(
            UserId::new(decode_id(
                &row.try_get::<String>("", "black").map_err(corrupt)?,
            )?)
            .map_err(corrupt)?,
            UserId::new(decode_id(
                &row.try_get::<String>("", "white").map_err(corrupt)?,
            )?)
            .map_err(corrupt)?,
        )
        .map_err(corrupt)?;
        let visibility = match row
            .try_get::<String>("", "visibility")
            .map_err(corrupt)?
            .as_str()
        {
            "public" => Visibility::Public,
            "private" => Visibility::Private,
            _ => return Err(Error::Corrupt),
        };
        let access = ViewMetadata {
            participants,
            visibility,
        };
        if !access.permits(viewer) {
            return Err(Error::NotFound);
        }
        Ok(access)
    }
    async fn version(&self, id: &GameId) -> Result<u64, Error> {
        let row = self
            .db
            .query_one(
                Query::select()
                    .column(a("version"))
                    .from(a("games"))
                    .and_where(Expr::col(a("id")).eq(encode_id(id.as_str()))),
            )
            .await
            .map_err(storage)?
            .ok_or(Error::NotFound)?;
        unsigned(row.try_get("", "version").map_err(corrupt)?)
    }

    async fn load_game(&self, id: &GameId) -> Result<GameRecord, Error> {
        Ok(Self::load_view(&self.db, id, None).await?.record)
    }

    async fn begin_game_update(
        &self,
        id: &GameId,
        terminal: bool,
    ) -> Result<Box<dyn LockedGameUpdate>, Error> {
        let tx = self.db.begin().await.map_err(storage)?;
        if terminal {
            matchmaking::admission_lock(&tx).await?;
        }
        tx.query_one(
            Query::select()
                .column(a("id"))
                .from(a("games"))
                .and_where(Expr::col(a("id")).eq(encode_id(id.as_str())))
                .lock(sea_orm::sea_query::LockType::Update),
        )
        .await
        .map_err(storage)?
        .ok_or(Error::NotFound)?;
        let record = Self::load_view(&tx, id, None).await?.record;
        Ok(Box::new(PgGameUpdate {
            tx,
            record,
            terminal,
        }))
    }
    async fn expired_games(&self, limit: u32) -> Result<Vec<GameId>, Error> {
        if !(1..=1000).contains(&limit) {
            return Err(Error::Internal);
        }
        let cutoff = db_now(&self.db).await?;
        self.db
            .query_all(
                Query::select()
                    .column(a("id"))
                    .from(a("games"))
                    .and_where(Expr::col(a("winner")).is_null())
                    .and_where(Expr::col(a("clock_deadline")).lte(cutoff.get()))
                    .order_by(a("clock_deadline"), Order::Asc)
                    .limit(u64::from(limit)),
            )
            .await
            .map_err(storage)?
            .into_iter()
            .map(|row| {
                GameId::new(decode_id(
                    &row.try_get::<String>("", "id").map_err(corrupt)?,
                )?)
                .map_err(corrupt)
            })
            .collect()
    }
}

fn database_time() -> Expr {
    Expr::cust("floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint")
}
async fn db_now(db: &impl ConnectionTrait) -> Result<Timestamp, Error> {
    Timestamp::new(
        db.query_one(Query::select().expr_as(database_time(), a("now")))
            .await
            .map_err(storage)?
            .ok_or(Error::Corrupt)?
            .try_get("", "now")
            .map_err(corrupt)?,
    )
}
fn decode_player(value: i32) -> Result<Player, Error> {
    match value {
        1 => Ok(Player::Black),
        2 => Ok(Player::White),
        _ => Err(Error::Corrupt),
    }
}
pub(crate) fn clock_columns() -> [&'static str; 7] {
    [
        "clock_black",
        "clock_white",
        "clock_started",
        "clock_deadline",
        "clock_stopped",
        "byoyomi_black",
        "byoyomi_white",
    ]
}
pub(crate) fn encode_clock(clock: ClockState) -> [Value; 7] {
    let none: Value = Option::<i64>::None.into();
    match clock {
        ClockState::Running {
            main_remaining_ms: m,
            turn_started_at: s,
            deadline_at: d,
        } => [
            m[0].get().into(),
            m[1].get().into(),
            s.get().into(),
            d.get().into(),
            none.clone(),
            none.clone(),
            none,
        ],
        ClockState::Stopped {
            main_remaining_ms: m,
            byoyomi_remaining_ms: b,
            stopped_at: s,
        } => [
            m[0].get().into(),
            m[1].get().into(),
            none.clone(),
            none,
            s.get().into(),
            b[0].get().into(),
            b[1].get().into(),
        ],
    }
}
pub(crate) fn decode_clock(row: &QueryResult, prefix: &str) -> Result<ClockState, Error> {
    let get = |name: &str| {
        row.try_get::<Option<i64>>("", &format!("{prefix}{name}"))
            .map_err(corrupt)
    };
    let main = [
        Milliseconds::new(get("clock_black")?.ok_or(Error::Corrupt)?)?,
        Milliseconds::new(get("clock_white")?.ok_or(Error::Corrupt)?)?,
    ];
    match (
        get("clock_started")?,
        get("clock_deadline")?,
        get("clock_stopped")?,
        get("byoyomi_black")?,
        get("byoyomi_white")?,
    ) {
        (Some(s), Some(d), None, None, None) => Ok(ClockState::Running {
            main_remaining_ms: main,
            turn_started_at: Timestamp::new(s)?,
            deadline_at: Timestamp::new(d)?,
        }),
        (None, None, Some(s), Some(b), Some(w)) => Ok(ClockState::Stopped {
            main_remaining_ms: main,
            byoyomi_remaining_ms: [Milliseconds::new(b)?, Milliseconds::new(w)?],
            stopped_at: Timestamp::new(s)?,
        }),
        _ => Err(Error::Corrupt),
    }
}
struct PgGameUpdate {
    tx: sea_orm::DatabaseTransaction,
    record: GameRecord,
    terminal: bool,
}
#[async_trait]
impl LockedGameUpdate for PgGameUpdate {
    fn locked_record(&self) -> &GameRecord {
        &self.record
    }
    async fn decision_time(&mut self) -> Result<Timestamp, Error> {
        Ok(db_now(&self.tx).await?.max(self.record.clock.saved_at()))
    }
    fn terminal_allowed(&self) -> bool {
        self.terminal
    }
    async fn rollback(self: Box<Self>) -> Result<(), Error> {
        self.tx.rollback().await.map_err(storage)
    }
    async fn commit(self: Box<Self>, change: &StagedGameChange) -> Result<(), Error> {
        let entry = change.entry();
        let acceptance = entry.acceptance();
        if change.game_id() != &self.record.id {
            return Err(Error::Corrupt);
        }
        if entry.expected_seq() != self.record.version {
            return Err(Error::SequenceMismatch {
                expected: entry.expected_seq(),
                actual: self.record.version,
            });
        }
        if acceptance.result.is_some() && !self.terminal {
            return Err(Error::Internal);
        }
        if let Some(r) = entry.request() {
            self.record.participants.side(&r.user_id)?;
        }
        let tx = self.tx;
        let seq = signed(acceptance.seq)?;
        let (winner, reason) = result_values(acceptance.result);
        let finished_at = match acceptance.clock {
            ClockState::Stopped { stopped_at, .. } => {
                Some(stopped_at.get().checked_mul(1000).ok_or(Error::Corrupt)?)
            }
            _ => None,
        };
        let mut update = Query::update();
        update
            .table(a("games"))
            .value(a("version"), seq)
            .value(a("winner"), winner.clone())
            .value(a("reason"), reason.clone())
            .value(a("finished_at"), finished_at)
            .value(
                a("final_move_ply"),
                Expr::col(a("final_move_ply")).add(i64::from(matches!(
                    entry.request().map(|r| r.action),
                    Some(Action::Move(_))
                ))),
            )
            .and_where(Expr::col(a("id")).eq(encode_id(change.game_id().as_str())));
        for (column, value) in clock_columns()
            .into_iter()
            .zip(encode_clock(acceptance.clock))
        {
            update.value(a(column), value);
        }
        tx.execute(&update).await.map_err(storage)?;
        let (request_id, user_id, action, usi, loser, deadline): (
            Option<String>,
            Option<String>,
            i32,
            Option<String>,
            Option<i32>,
            Option<i64>,
        ) = match entry {
            JournalEntry::UserCommand(r) => {
                let (action, usi) = match r.request.action {
                    Action::Move(m) => (1, Some(m.to_string())),
                    Action::Resign => (2, None),
                };
                (
                    Some(encode_id(r.request.request_id.as_str())),
                    Some(encode_id(r.request.user_id.as_str())),
                    action,
                    usi,
                    None,
                    None,
                )
            }
            JournalEntry::Timeout {
                loser, deadline_at, ..
            } => (
                None,
                None,
                3,
                None,
                Some(if *loser == Player::Black { 1 } else { 2 }),
                Some(deadline_at.get()),
            ),
        };
        let mut insert = Query::insert();
        insert.into_table(a("journal")).columns(
            [
                "game_id",
                "seq",
                "request_id",
                "user_id",
                "expected_seq",
                "action",
                "usi",
                "in_check",
                "winner",
                "reason",
                "evaluated_at",
                "loser",
                "deadline_at",
            ]
            .into_iter()
            .chain(clock_columns())
            .map(a),
        );
        insert.values_panic(
            vec![
                encode_id(change.game_id().as_str()).into(),
                seq.into(),
                request_id.into(),
                user_id.into(),
                signed(entry.expected_seq())?.into(),
                action.into(),
                usi.into(),
                acceptance.in_check.into(),
                winner,
                reason,
                entry.evaluated_at().get().into(),
                loser.into(),
                deadline.into(),
            ]
            .into_iter()
            .chain(encode_clock(acceptance.clock))
            .map(Expr::from),
        );
        tx.execute(&insert).await.map_err(storage)?;
        for event in change.events() {
            outbox::enqueue(&tx, event).await?;
        }
        if acceptance.result.is_some() {
            matchmaking::finish(
                &tx,
                change.game_id(),
                self.record.participants.black().as_str(),
                self.record.participants.white().as_str(),
            )
            .await?;
            rating::enqueue(&tx, change.game_id()).await?;
        }
        tx.commit().await.map_err(storage)
    }
}
