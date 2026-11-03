use crate::{
    Error, SeaOrmStore, corrupt, result, result_values, schema::a, signed, storage, unsigned,
};
use async_trait::async_trait;
use game_application::{
    CommittedEvent, DomainEvent, GameId, OutboxItem, OutboxRepository, RequestId, UserId,
};
use sea_orm::sea_query::{Expr, ExprTrait, Order, Query, Value};
use sea_orm::{ConnectionTrait, DatabaseTransaction};

/// Failure opening a publisher's exclusive SQLite queue.
#[derive(Debug)]
pub enum OutboxOpenError {
    /// Database connection, schema initialization or validation, or filename lookup failed.
    Database(Error),
    /// The SQLite database has no local backing file.
    FileRequired,
    /// Opening or exclusively locking the database failed, including a competing publisher.
    Lock(std::io::Error),
}
impl std::fmt::Display for OutboxOpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Database(e) => write!(f, "outbox database: {e}"),
            Self::FileRequired => {
                f.write_str("outbox requires a file-backed SQLite database on Linux local storage")
            }
            Self::Lock(e) => write!(f, "outbox exclusive database lock: {e}"),
        }
    }
}
impl std::error::Error for OutboxOpenError {}

/// Storage-owned queue cursor. It is not a public event identity.
pub struct SqliteCursor(i64);

/// Exclusive pending-event access, supported on Linux local filesystems.
/// The non-cloneable guard owns an OS lock on the real database inode. SQLite's
/// byte-range transaction locks remain independent of this publisher lock.
/// Configure all processes with the same database path. Hardlink aliases are
/// unsupported because SQLite would use different WAL paths. Do not unlink or
/// replace a live database file.
pub struct SqliteOutbox {
    store: SeaOrmStore,
    _lock: std::fs::File,
}
impl SqliteOutbox {
    /// Opens and initializes or validates the database, then refuses a competing publisher.
    ///
    /// Returns an error for an in-memory database, a competing lock, or storage failure.
    pub async fn connect(url: &str) -> Result<Self, OutboxOpenError> {
        if !cfg!(target_os = "linux") {
            return Err(OutboxOpenError::FileRequired);
        }
        let store = SeaOrmStore::connect(url)
            .await
            .map_err(OutboxOpenError::Database)?;
        let rows = store
            .db
            .query_all_raw(sea_orm::Statement::from_string(
                sea_orm::DbBackend::Sqlite,
                "PRAGMA database_list",
            ))
            .await
            .map_err(|e| OutboxOpenError::Database(storage(e)))?;
        let filename = rows
            .iter()
            .find_map(|row| {
                (row.try_get::<String>("", "name").ok()?.as_str() == "main")
                    .then(|| row.try_get::<String>("", "file").ok())
                    .flatten()
            })
            .filter(|s| !s.is_empty())
            .ok_or(OutboxOpenError::FileRequired)?;
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(filename)
            .map_err(OutboxOpenError::Lock)?;
        lock.try_lock()
            .map_err(|e| OutboxOpenError::Lock(e.into()))?;
        Ok(Self { store, _lock: lock })
    }
}

pub(crate) async fn enqueue(tx: &DatabaseTransaction, event: &CommittedEvent) -> Result<(), Error> {
    let (kind, usi, in_check, terminal) = match event.event {
        DomainEvent::MoveCommitted {
            movement,
            in_check,
            result,
        } => (0, Some(movement.to_string()), Some(in_check), result),
        DomainEvent::GameFinished { result } => (1, None, None, Some(result)),
    };
    let (winner, reason) = result_values(terminal);
    tx.execute(
        Query::insert()
            .into_table(a("outbox_events"))
            .columns([
                a("game_id"),
                a("seq"),
                a("event_kind"),
                a("encoding_version"),
                a("request_id"),
                a("user_id"),
                a("usi"),
                a("in_check"),
                a("winner"),
                a("reason"),
            ])
            .values_panic([
                event.game_id.as_str().into(),
                signed(event.seq)?.into(),
                kind.into(),
                1.into(),
                event.request_id.as_str().into(),
                event.user_id.as_str().into(),
                usi.into(),
                in_check.into(),
                winner.into(),
                reason.into(),
            ]),
    )
    .await
    .map_err(storage)?;
    Ok(())
}

#[async_trait]
impl OutboxRepository for SqliteOutbox {
    type Cursor = SqliteCursor;
    async fn next_pending(&mut self) -> Result<Option<OutboxItem<SqliteCursor>>, Error> {
        let Some(row) = self
            .store
            .db
            .query_one(
                Query::select()
                    .columns([
                        a("id"),
                        a("game_id"),
                        a("seq"),
                        a("event_kind"),
                        a("encoding_version"),
                        a("request_id"),
                        a("user_id"),
                        a("usi"),
                        a("in_check"),
                        a("winner"),
                        a("reason"),
                    ])
                    .from(a("outbox_events"))
                    .and_where(Expr::col(a("published")).eq(false))
                    .order_by(a("id"), Order::Asc)
                    .limit(1),
            )
            .await
            .map_err(storage)?
        else {
            return Ok(None);
        };
        if row
            .try_get::<i32>("", "encoding_version")
            .map_err(corrupt)?
            != 1
        {
            return Err(Error::Corrupt);
        }
        let usi: Option<String> = row.try_get("", "usi").map_err(corrupt)?;
        let in_check: Option<bool> = row.try_get("", "in_check").map_err(corrupt)?;
        let terminal = result(&row, "")?;
        let event = match (
            row.try_get::<i32>("", "event_kind").map_err(corrupt)?,
            usi,
            in_check,
        ) {
            (0, Some(usi), Some(in_check))
                if terminal.is_none_or(|r| r.reason == shogi_domain::FinishReason::Checkmate) =>
            {
                DomainEvent::MoveCommitted {
                    movement: usi.parse().map_err(corrupt)?,
                    in_check,
                    result: terminal,
                }
            }
            (1, None, None) => DomainEvent::GameFinished {
                result: terminal.ok_or(Error::Corrupt)?,
            },
            _ => return Err(Error::Corrupt),
        };
        Ok(Some(OutboxItem {
            cursor: SqliteCursor(row.try_get("", "id").map_err(corrupt)?),
            event: CommittedEvent {
                game_id: GameId::new(row.try_get::<String>("", "game_id").map_err(corrupt)?)
                    .map_err(corrupt)?,
                request_id: RequestId::new(
                    row.try_get::<String>("", "request_id").map_err(corrupt)?,
                )
                .map_err(corrupt)?,
                user_id: UserId::new(row.try_get::<String>("", "user_id").map_err(corrupt)?)
                    .map_err(corrupt)?,
                seq: unsigned(row.try_get("", "seq").map_err(corrupt)?)?,
                event,
            },
        }))
    }
    async fn mark_published(&mut self, item: &OutboxItem<SqliteCursor>) -> Result<(), Error> {
        self.store
            .db
            .execute(
                Query::update()
                    .table(a("outbox_events"))
                    .values([(a("published"), Value::Bool(Some(true)).into())])
                    .and_where(Expr::col(a("id")).eq(item.cursor.0)),
            )
            .await
            .map_err(storage)?;
        Ok(())
    }
}
