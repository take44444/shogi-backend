//! SQLite persistence for the application-owned [`Store`] contract through SeaORM.
//!
//! [`SeaOrmStore::connect`] opens a pool and migrates or validates the schema before
//! returning. SQLite is the only compiled and tested driver. Other adapters or
//! drivers require their own validation, even though the application port is neutral.
//!
//! Each connection enables foreign keys, full synchronization, and a five-second
//! busy timeout. File databases use WAL; in-memory databases use memory journaling.
//! In-memory databases use one pooled connection; file databases use
//! up to five. Creation and append use immediate SQLite transactions. Append compares
//! the version and writes metadata and the receipt atomically, checking membership
//! before commit. Failed append statements explicitly roll back. Commit errors can be
//! uncertain; the application reconciles every append error through a fresh load.
//!
//! Metadata and ordered receipts load through one left join, so a read cannot combine
//! different database snapshots. The application validates replay from a standard
//! game rather than restoring an untrusted position string. Manual database edits
//! are outside the mutation API.
//!
//! ```
//! use game_application::{GameId, GameService, MakeMove, Participants, RequestId, UserId};
//! use persistence::SeaOrmStore;
//! use std::sync::Arc;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let store = Arc::new(SeaOrmStore::connect("sqlite::memory:").await?);
//! let service = GameService::new(store.clone());
//! let black = UserId::new("black")?;
//! let white = UserId::new("white")?;
//! service.create_user(black.clone()).await?;
//! service.create_user(white.clone()).await?;
//! let id = GameId::new("example")?;
//! assert_eq!(service.create(id.clone(), Participants { black: black.clone(), white }).await?.seq, 0);
//! let command = MakeMove {
//!     request_id: RequestId::new("opening")?, expected_seq: 0,
//!     user_id: black, movement: "7g7f".parse()?,
//! };
//! let accepted = service.make_move(&id, command.clone()).await?;
//! assert_eq!(accepted.seq, 1);
//! assert_eq!(service.make_move(&id, command).await?, accepted);
//! assert_eq!(service.snapshot(&id).await?.seq, 1);
//! store.close().await?;
//! # Ok(())
//! # }
//! ```
#![warn(missing_docs)]

mod schema;

use async_trait::async_trait;
use game_application::{
    Acceptance, Action, ApplicationError as Error, GameId, GameRecord, Participants, Receipt,
    RequestId, Store, User, UserId,
};
use schema::a;
use sea_orm::sea_query::{Expr, ExprTrait, JoinType, Order, Query, Value};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbErr, QueryResult,
    SqliteTransactionMode, TransactionOptions, TransactionTrait,
};
use shogi_domain::{FinishReason, GameResult, Player};
use std::time::Duration;

/// A migrated SQLite connection pool implementing [`Store`].
/// Share through `Arc` when multiple services use the same pool.
pub struct SeaOrmStore {
    db: DatabaseConnection,
}

impl SeaOrmStore {
    /// Opens SQLite and validates or migrates its schema before returning.
    /// For a new database, creates the schema and version ledger atomically.
    /// Existing empty or incompatible ledgers and missing application tables fail
    /// closed. Concurrent startup is serialized by an immediate transaction.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unavailable`] for connection or database errors and
    /// [`Error::Corrupt`] for an incompatible or incomplete schema.
    pub async fn connect(url: &str) -> Result<Self, Error> {
        let mut options = ConnectOptions::new(url);
        options
            .max_connections(if url.contains(":memory:") { 1 } else { 5 })
            .sqlx_logging(false)
            .map_sqlx_sqlite_opts(|options| {
                options
                    .foreign_keys(true)
                    .pragma("journal_mode", "WAL")
                    .pragma("synchronous", "FULL")
                    .busy_timeout(Duration::from_secs(5))
            });
        let db = Database::connect(options).await.map_err(storage)?;
        schema::migrate(&db).await.map_err(|error| match error {
            schema::MigrationError::Corrupt => Error::Corrupt,
            schema::MigrationError::Database(error) => storage(error),
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
impl Store for SeaOrmStore {
    async fn create_user(&self, id: UserId) -> Result<User, Error> {
        match self
            .db
            .execute(
                Query::insert()
                    .into_table(a("users"))
                    .columns([a("id")])
                    .values_panic([id.as_str().into()]),
            )
            .await
        {
            Ok(_) => Ok(User { id }),
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

    async fn get_user(&self, id: &UserId) -> Result<User, Error> {
        self.db
            .query_one(
                Query::select()
                    .column(a("id"))
                    .from(a("users"))
                    .and_where(Expr::col(a("id")).eq(id.as_str())),
            )
            .await
            .map_err(storage)?
            .ok_or(Error::UserNotFound)?;
        Ok(User { id: id.clone() })
    }

    async fn create_game(&self, id: GameId, participants: Participants) -> Result<(), Error> {
        if participants.black == participants.white {
            return Err(Error::InvalidParticipants);
        }
        let tx = self
            .db
            .begin_with_options(TransactionOptions {
                sqlite_transaction_mode: Some(SqliteTransactionMode::Immediate),
                ..Default::default()
            })
            .await
            .map_err(storage)?;
        for user in [&participants.black, &participants.white] {
            if tx
                .query_one(
                    Query::select()
                        .column(a("id"))
                        .from(a("users"))
                        .and_where(Expr::col(a("id")).eq(user.as_str())),
                )
                .await
                .map_err(storage)?
                .is_none()
            {
                return Err(Error::UserNotFound);
            }
        }
        let inserted = tx
            .execute(
                Query::insert()
                    .into_table(a("games"))
                    .columns([a("id"), a("black"), a("white"), a("version")])
                    .values_panic([
                        id.as_str().into(),
                        participants.black.as_str().into(),
                        participants.white.as_str().into(),
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
                    .and_where(Expr::col(a("id")).eq(id.as_str())),
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
            .and_where(Expr::col((a("games"), a("id"))).eq(id.as_str()))
            .order_by((a("journal"), a("seq")), Order::Asc);
        let rows = self.db.query_all(&query).await.map_err(storage)?;
        let first = rows.first().ok_or(Error::NotFound)?;
        let mut record = GameRecord {
            id: GameId::new(first.try_get::<String>("", "g_id").map_err(corrupt)?)
                .map_err(corrupt)?,
            participants: Participants {
                black: UserId::new(first.try_get::<String>("", "g_black").map_err(corrupt)?)
                    .map_err(corrupt)?,
                white: UserId::new(first.try_get::<String>("", "g_white").map_err(corrupt)?)
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
                request_id: RequestId::new(
                    row.try_get::<String>("", "request_id").map_err(corrupt)?,
                )
                .map_err(corrupt)?,
                user_id: UserId::new(row.try_get::<String>("", "user_id").map_err(corrupt)?)
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

    async fn append(&self, id: &GameId, receipt: &Receipt) -> Result<(), Error> {
        let expected = signed(receipt.expected_seq)?;
        let seq = signed(receipt.acceptance.seq)?;
        if expected == i64::MAX {
            return Err(Error::SequenceExhausted);
        }
        if seq != expected + 1 {
            return Err(Error::Corrupt);
        }
        let tx = self
            .db
            .begin_with_options(TransactionOptions {
                sqlite_transaction_mode: Some(SqliteTransactionMode::Immediate),
                ..Default::default()
            })
            .await
            .map_err(storage)?;
        let operation = async {
            let (winner, reason) = result_values(receipt.acceptance.result);
            let updated = tx
                .execute(
                    Query::update()
                        .table(a("games"))
                        .values([
                            (a("version"), seq.into()),
                            (a("winner"), winner.clone().into()),
                            (a("reason"), reason.clone().into()),
                        ])
                        .and_where(Expr::col(a("id")).eq(id.as_str()))
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
                            .and_where(Expr::col(a("id")).eq(id.as_str())),
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
                        .and_where(Expr::col(a("id")).eq(id.as_str())),
                )
                .await
                .map_err(storage)?
                .ok_or(Error::NotFound)?;
            let black: String = row.try_get("", "black").map_err(corrupt)?;
            let white: String = row.try_get("", "white").map_err(corrupt)?;
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
                        id.as_str().into(),
                        seq.into(),
                        receipt.request_id.as_str().into(),
                        receipt.user_id.as_str().into(),
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
