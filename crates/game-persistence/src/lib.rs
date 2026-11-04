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

mod outbox;
mod record;
mod schema;
pub use outbox::{OutboxOpenError, PostgresCursor, PostgresOutbox};
pub use record::PostgresGameRecords;

use async_trait::async_trait;
use game_application::{
    Acceptance, Action, ApplicationError as Error, GameId, GameRecord, GameRepository,
    Participants, Receipt, RequestId, StagedGameChange, UserId,
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
                _ => return Err(Error::Corrupt),
            },
        })),
        _ => Err(Error::Corrupt),
    }
}

#[async_trait]
impl GameRepository for SeaOrmStore {
    async fn create_game(&self, id: GameId, participants: Participants) -> Result<(), Error> {
        if participants.black == participants.white {
            return Err(Error::InvalidParticipants);
        }
        let tx = self.db.begin().await.map_err(storage)?;
        let inserted = tx
            .execute(
                Query::insert()
                    .into_table(a("games"))
                    .columns([
                        a("id"),
                        a("black"),
                        a("white"),
                        a("version"),
                        a("rule"),
                        a("visibility"),
                        a("final_move_ply"),
                    ])
                    .values_panic([
                        encode_id(id.as_str()).into(),
                        encode_id(participants.black.as_str()).into(),
                        encode_id(participants.white.as_str()).into(),
                        0_i64.into(),
                        "standard".into(),
                        "public".into(),
                        0_i64.into(),
                    ]),
            )
            .await;
        match inserted {
            Ok(_) => tx.commit().await.map_err(storage),
            Err(error)
                if error.sql_err().is_some_and(|e| {
                    matches!(e, sea_orm::SqlErr::UniqueConstraintViolation(_))
                }) =>
            {
                Err(Error::AlreadyExists)
            }
            Err(error) => Err(storage(error)),
        }
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
        let mut query = Query::select();
        for column in ["id", "black", "white", "version", "winner", "reason"] {
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
        let rows = self.db.query_all(&query).await.map_err(storage)?;
        let first = rows.first().ok_or(Error::NotFound)?;
        let mut record = GameRecord {
            id: GameId::new(decode_id(
                &first.try_get::<String>("", "g_id").map_err(corrupt)?,
            )?)
            .map_err(corrupt)?,
            participants: Participants {
                black: UserId::new(decode_id(
                    &first.try_get::<String>("", "g_black").map_err(corrupt)?,
                )?)
                .map_err(corrupt)?,
                white: UserId::new(decode_id(
                    &first.try_get::<String>("", "g_white").map_err(corrupt)?,
                )?)
                .map_err(corrupt)?,
            },
            version: unsigned(first.try_get("", "g_version").map_err(corrupt)?)?,
            result: result(first, "g_")?,
            journal: Vec::new(),
        };
        for row in rows {
            let seq: Option<i64> = row.try_get("", "seq").map_err(corrupt)?;
            let Some(seq) = seq else {
                continue;
            };
            let usi: Option<String> = row.try_get("", "usi").map_err(corrupt)?;
            let action = match (row.try_get::<i32>("", "action").map_err(corrupt)?, usi) {
                (1, Some(usi)) => Action::Move(usi.parse().map_err(corrupt)?),
                (2, None) => Action::Resign,
                _ => return Err(Error::Corrupt),
            };
            record.journal.push(Receipt {
                request_id: RequestId::new(decode_id(
                    &row.try_get::<String>("", "request_id").map_err(corrupt)?,
                )?)
                .map_err(corrupt)?,
                user_id: UserId::new(decode_id(
                    &row.try_get::<String>("", "user_id").map_err(corrupt)?,
                )?)
                .map_err(corrupt)?,
                expected_seq: unsigned(row.try_get("", "expected_seq").map_err(corrupt)?)?,
                action,
                acceptance: Acceptance {
                    seq: unsigned(seq)?,
                    in_check: row.try_get("", "in_check").map_err(corrupt)?,
                    result: result(&row, "")?,
                },
            });
        }
        Ok(record)
    }

    async fn commit_change(&self, change: &StagedGameChange) -> Result<(), Error> {
        let id = change.game_id();
        let receipt = change.receipt();
        let expected = signed(receipt.expected_seq)?;
        let seq = signed(receipt.acceptance.seq)?;
        let tx = self.db.begin().await.map_err(storage)?;
        let operation = async {
            let (winner, reason) = result_values(receipt.acceptance.result);
            let finished_at = receipt
                .acceptance
                .result
                .map(|_| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .and_then(|time| i64::try_from(time.as_micros()).ok())
                        .ok_or(Error::Unavailable)
                })
                .transpose()?;
            let updated = tx
                .execute(
                    Query::update()
                        .table(a("games"))
                        .value(a("version"), seq)
                        .value(a("winner"), winner.clone())
                        .value(a("reason"), reason.clone())
                        .value(
                            a("final_move_ply"),
                            Expr::col(a("final_move_ply"))
                                .add(i64::from(matches!(receipt.action, Action::Move(_)))),
                        )
                        .value(a("finished_at"), finished_at)
                        .and_where(Expr::col(a("id")).eq(encode_id(id.as_str())))
                        .and_where(Expr::col(a("version")).eq(expected)),
                )
                .await
                .map_err(storage)?;
            if updated.rows_affected() != 1 {
                let row = tx
                    .query_one(
                        Query::select()
                            .column(a("version"))
                            .from(a("games"))
                            .and_where(Expr::col(a("id")).eq(encode_id(id.as_str()))),
                    )
                    .await
                    .map_err(storage)?
                    .ok_or(Error::NotFound)?;
                return Err(Error::SequenceMismatch {
                    expected: receipt.expected_seq,
                    actual: unsigned(row.try_get("", "version").map_err(corrupt)?)?,
                });
            }
            let row = tx
                .query_one(
                    Query::select()
                        .columns([a("black"), a("white")])
                        .from(a("games"))
                        .and_where(Expr::col(a("id")).eq(encode_id(id.as_str()))),
                )
                .await
                .map_err(storage)?
                .ok_or(Error::NotFound)?;
            let black = decode_id(&row.try_get::<String>("", "black").map_err(corrupt)?)?;
            let white = decode_id(&row.try_get::<String>("", "white").map_err(corrupt)?)?;
            if receipt.user_id.as_str() != black && receipt.user_id.as_str() != white {
                return Err(Error::NonParticipant);
            }
            let (action, usi): (i32, Option<String>) = match receipt.action {
                Action::Move(movement) => (1, Some(movement.to_string())),
                Action::Resign => (2, None),
            };
            tx.execute(
                Query::insert()
                    .into_table(a("journal"))
                    .columns([
                        a("game_id"),
                        a("seq"),
                        a("request_id"),
                        a("user_id"),
                        a("expected_seq"),
                        a("action"),
                        a("usi"),
                        a("in_check"),
                        a("winner"),
                        a("reason"),
                    ])
                    .values_panic([
                        encode_id(id.as_str()).into(),
                        seq.into(),
                        encode_id(receipt.request_id.as_str()).into(),
                        encode_id(receipt.user_id.as_str()).into(),
                        expected.into(),
                        action.into(),
                        usi.into(),
                        receipt.acceptance.in_check.into(),
                        winner.into(),
                        reason.into(),
                    ]),
            )
            .await
            .map_err(storage)?;
            for event in change.events() {
                outbox::enqueue(&tx, event).await?;
            }
            Ok(())
        }
        .await;
        match operation {
            Ok(()) => tx.commit().await.map_err(storage),
            Err(error) => {
                tx.rollback().await.map_err(storage)?;
                Err(error)
            }
        }
    }
}
