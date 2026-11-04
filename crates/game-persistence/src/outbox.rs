use crate::{
    Error, SeaOrmStore, corrupt, decode_id, encode_id, result, result_values, schema::a, signed,
    storage, unsigned,
};
use async_trait::async_trait;
use game_application::{
    CommittedEvent, DomainEvent, GameId, OutboxItem, OutboxRepository, RequestId, UserId,
};
use sea_orm::sea_query::{Expr, ExprTrait, Order, PostgresQueryBuilder, Query, Value};
use sea_orm::sqlx::{self, AssertSqlSafe, Connection, PgConnection, postgres::PgConnectOptions};
use sea_orm::{ConnectionTrait, DatabaseTransaction};

/// Failure opening a publisher's exclusive PostgreSQL queue.
#[derive(Debug)]
pub enum OutboxOpenError {
    /// Database connection, schema initialization, or validation failed.
    Database(Error),
    /// Another publisher owns this database's advisory lock.
    PublisherAlreadyRunning,
}
impl std::fmt::Display for OutboxOpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Database(e) => write!(f, "outbox database: {e}"),
            Self::PublisherAlreadyRunning => f.write_str("another outbox publisher is running"),
        }
    }
}
impl std::error::Error for OutboxOpenError {}

/// Storage-owned queue cursor. It is not a public event identity.
pub struct PostgresCursor(i64);

/// Exclusive pending-event access through a dedicated PostgreSQL session.
/// The connection owns a session advisory lock and never reconnects. Queue reads
/// and acknowledgments use that same session, so losing it fails closed. No
/// transaction remains open while a caller publishes an event over the network.
pub struct PostgresOutbox {
    connection: PgConnection,
}
impl PostgresOutbox {
    /// Initializes or validates the database, then acquires its publisher lock.
    ///
    /// # Errors
    ///
    /// Returns an error for a competing publisher or a storage failure.
    pub async fn connect(url: &str) -> Result<Self, OutboxOpenError> {
        let store = SeaOrmStore::connect(url)
            .await
            .map_err(OutboxOpenError::Database)?;
        store.close().await.map_err(OutboxOpenError::Database)?;
        let options: PgConnectOptions = url
            .parse()
            .map_err(|_| OutboxOpenError::Database(Error::Unavailable))?;
        let mut connection = PgConnection::connect_with(
            &options
                .application_name("shogi-outbox-publisher")
                .options([("search_path", "public")]),
        )
        .await
        .map_err(|_| OutboxOpenError::Database(Error::Unavailable))?;
        let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(6001133723894506836)")
            .fetch_one(&mut connection)
            .await
            .map_err(|_| OutboxOpenError::Database(Error::Unavailable))?;
        if !locked {
            return Err(OutboxOpenError::PublisherAlreadyRunning);
        }
        Ok(Self { connection })
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
                encode_id(event.game_id.as_str()).into(),
                signed(event.seq)?.into(),
                kind.into(),
                1.into(),
                encode_id(event.request_id.as_str()).into(),
                encode_id(event.user_id.as_str()).into(),
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
impl OutboxRepository for PostgresOutbox {
    type Cursor = PostgresCursor;
    async fn next_pending(&mut self) -> Result<Option<OutboxItem<PostgresCursor>>, Error> {
        let query = Query::select()
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
            .limit(1)
            .to_string(PostgresQueryBuilder);
        let Some(row) = sqlx::query(AssertSqlSafe(query))
            .fetch_optional(&mut self.connection)
            .await
            .map_err(|_| Error::Unavailable)?
        else {
            return Ok(None);
        };
        let row: sea_orm::QueryResult = row.into();
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
            cursor: PostgresCursor(row.try_get("", "id").map_err(corrupt)?),
            event: CommittedEvent {
                game_id: GameId::new(decode_id(
                    &row.try_get::<String>("", "game_id").map_err(corrupt)?,
                )?)
                .map_err(corrupt)?,
                request_id: RequestId::new(decode_id(
                    &row.try_get::<String>("", "request_id").map_err(corrupt)?,
                )?)
                .map_err(corrupt)?,
                user_id: UserId::new(decode_id(
                    &row.try_get::<String>("", "user_id").map_err(corrupt)?,
                )?)
                .map_err(corrupt)?,
                seq: unsigned(row.try_get("", "seq").map_err(corrupt)?)?,
                event,
            },
        }))
    }
    async fn mark_published(&mut self, item: &OutboxItem<PostgresCursor>) -> Result<(), Error> {
        let query = Query::update()
            .table(a("outbox_events"))
            .values([(a("published"), Value::Bool(Some(true)).into())])
            .and_where(Expr::col(a("id")).eq(item.cursor.0))
            .to_string(PostgresQueryBuilder);
        sqlx::query(AssertSqlSafe(query))
            .execute(&mut self.connection)
            .await
            .map_err(|_| Error::Unavailable)?;
        Ok(())
    }
}
