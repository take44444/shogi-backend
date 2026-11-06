use crate::schema::a;
use async_trait::async_trait;
use game_record_application::{
    Detail, Error, GameId, MAX_HISTORY, Metadata, Page, PublicAction, PublicOperation,
    RecordRepository, Rule, Search, UserId,
};
use sea_orm::sea_query::{
    Cond, Expr, ExprTrait, Order, PostgresQueryBuilder, Query, SelectStatement,
};
use sea_orm::sqlx::{
    self, AssertSqlSafe, PgPool, Postgres, Row,
    pool::PoolConnection,
    postgres::{PgConnectOptions, PgPoolOptions, PgRow},
};
use sea_query_sqlx::SqlxBinder;
use shogi_domain::{FinishReason, GameResult, Player};
use std::{str::FromStr, time::Duration};

fn metadata_query() -> SelectStatement {
    Query::select()
        .columns(
            [
                "id",
                "black",
                "white",
                "version",
                "winner",
                "reason",
                "rule",
                "finished_at",
                "final_move_ply",
            ]
            .map(a),
        )
        .from(a("games"))
        .and_where(Expr::col(a("visibility")).eq("public"))
        .and_where(Expr::col(a("finished_at")).is_not_null())
        .to_owned()
}

/// Read-only record access without schema initialization or game mutation.
pub struct PostgresGameRecords {
    pool: PgPool,
}
impl PostgresGameRecords {
    /// Connects to an existing schema-v5 database without initializing or mutating it.
    /// Use a SELECT-only database role to enforce permissions independently of this adapter.
    /// Each connection uses read-only transactions and a two-second statement timeout.
    ///
    /// # Errors
    /// Returns `Unavailable` for connection failures and `Corrupt` for an unsupported schema.
    pub async fn connect(url: &str) -> Result<Self, Error> {
        let options = PgConnectOptions::from_str(url)
            .map_err(unavailable)?
            .options([("search_path", "public")]);
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(2))
            .after_connect(|connection, _| {
                Box::pin(async move {
                    for command in [
                        "SET default_transaction_read_only = on",
                        "SET statement_timeout = '2s'",
                        "SET lock_timeout = '2s'",
                    ] {
                        sqlx::query(command).execute(&mut *connection).await?;
                    }
                    Ok(())
                })
            })
            .connect_with(options)
            .await
            .map_err(unavailable)?;
        let reader = Self { pool };
        let mut connection = reader.connection().await?;
        validate_schema(&mut connection).await?;
        Ok(reader)
    }
    /// Closes the reader pool and makes later queries unavailable.
    pub async fn close(&self) {
        self.pool.close().await;
    }
    async fn connection(&self) -> Result<PoolConnection<Postgres>, Error> {
        self.pool.acquire().await.map_err(unavailable)
    }
}
fn unavailable(_: impl std::fmt::Debug) -> Error {
    Error::Unavailable
}
fn corrupt(_: impl std::fmt::Debug) -> Error {
    Error::Corrupt
}
async fn validate_schema(connection: &mut PoolConnection<Postgres>) -> Result<(), Error> {
    let (sql, values) = Query::select()
        .columns([a("id"), a("version")])
        .from(a("schema_version"))
        .build_sqlx(PostgresQueryBuilder);
    let rows = sqlx::query_with(AssertSqlSafe(sql), values)
        .fetch_all(&mut **connection)
        .await
        .map_err(|_| Error::Corrupt)?;
    if rows.len() != 1
        || rows[0].try_get::<i64, _>("id").map_err(corrupt)? != 0
        || rows[0].try_get::<i64, _>("version").map_err(corrupt)? != 5
    {
        return Err(Error::Corrupt);
    }
    for (table, columns) in [
        (
            "games",
            &[
                ("id", "TEXT"),
                ("black", "TEXT"),
                ("white", "TEXT"),
                ("version", "BIGINT"),
                ("winner", "INTEGER"),
                ("reason", "INTEGER"),
                ("rule", "TEXT"),
                ("visibility", "TEXT"),
                ("finished_at", "BIGINT"),
                ("final_move_ply", "BIGINT"),
            ][..],
        ),
        (
            "journal",
            &[
                ("game_id", "TEXT"),
                ("seq", "BIGINT"),
                ("request_id", "TEXT"),
                ("user_id", "TEXT"),
                ("expected_seq", "BIGINT"),
                ("action", "INTEGER"),
                ("usi", "TEXT"),
                ("in_check", "BOOLEAN"),
                ("winner", "INTEGER"),
                ("reason", "INTEGER"),
            ][..],
        ),
    ] {
        let rows = sqlx::query("SELECT column_name AS name, data_type AS type, is_nullable FROM information_schema.columns WHERE table_schema = current_schema() AND table_name = $1")
            .bind(table)
            .fetch_all(&mut **connection)
            .await
            .map_err(corrupt)?;
        if rows.len() != columns.len()
            || columns.iter().any(|(name, kind)| {
                !rows.iter().any(|row| {
                    row.try_get::<String, _>("name").is_ok_and(|s| s == *name)
                        && row
                            .try_get::<String, _>("type")
                            .is_ok_and(|s| s.eq_ignore_ascii_case(kind))
                })
            })
        {
            return Err(Error::Corrupt);
        }
        if table == "games"
            && ["rule", "visibility", "final_move_ply"].iter().any(|name| {
                !rows.iter().any(|row| {
                    row.try_get::<String, _>("name")
                        .is_ok_and(|value| value == *name)
                        && row
                            .try_get::<String, _>("is_nullable")
                            .is_ok_and(|value| value == "NO")
                })
            })
        {
            return Err(Error::Corrupt);
        }
        let kind = sqlx::query("SELECT table_type AS type FROM information_schema.tables WHERE table_schema = current_schema() AND table_name = $1")
            .bind(table)
            .fetch_one(&mut **connection)
            .await
            .map_err(corrupt)?;
        if kind.try_get::<String, _>("type").map_err(corrupt)? != "BASE TABLE" {
            return Err(Error::Corrupt);
        }
    }
    Ok(())
}
fn side(value: i32) -> Result<Player, Error> {
    match value {
        1 => Ok(Player::Black),
        2 => Ok(Player::White),
        _ => Err(Error::Corrupt),
    }
}
fn reason(value: i32) -> Result<FinishReason, Error> {
    match value {
        1 => Ok(FinishReason::Checkmate),
        2 => Ok(FinishReason::Resignation),
        _ => Err(Error::Corrupt),
    }
}
fn number(row: &PgRow, column: &str) -> Result<u64, Error> {
    u64::try_from(row.try_get::<i64, _>(column).map_err(corrupt)?).map_err(corrupt)
}
fn metadata(row: &PgRow) -> Result<Metadata, Error> {
    let data = Metadata {
        id: GameId::new(decode(row.try_get::<String, _>("id").map_err(corrupt)?)?)
            .map_err(corrupt)?,
        black: UserId::new(decode(row.try_get::<String, _>("black").map_err(corrupt)?)?)
            .map_err(corrupt)?,
        white: UserId::new(decode(row.try_get::<String, _>("white").map_err(corrupt)?)?)
            .map_err(corrupt)?,
        rule: Rule::Standard,
        result: GameResult {
            winner: side(row.try_get("winner").map_err(corrupt)?)?,
            reason: reason(row.try_get("reason").map_err(corrupt)?)?,
        },
        finished_at: row.try_get("finished_at").map_err(corrupt)?,
        final_seq: number(row, "version")?,
        final_move_ply: number(row, "final_move_ply")?,
    };
    if data.black == data.white
        || data.finished_at < 0
        || data.final_seq == 0
        || data.final_move_ply
            != data.final_seq - u64::from(data.result.reason == FinishReason::Resignation)
    {
        return Err(Error::Corrupt);
    }
    Ok(data)
}
#[async_trait]
impl RecordRepository for PostgresGameRecords {
    async fn search(&self, search: &Search) -> Result<Page, Error> {
        let f = search.filters();
        let mut query = metadata_query();
        query
            .and_where(Expr::col(a("finished_at")).gte(f.from))
            .and_where(Expr::col(a("finished_at")).lt(f.to));
        if let Some(id) = &f.participant {
            query.cond_where(
                Cond::any()
                    .add(Expr::col(a("black")).eq(crate::encode_id(id.as_str())))
                    .add(Expr::col(a("white")).eq(crate::encode_id(id.as_str()))),
            );
        }
        if let Some(winner) = f.winner {
            query.and_where(Expr::col(a("winner")).eq(if winner == Player::Black {
                1i32
            } else {
                2
            }));
        }
        if let Some(reason) = f.reason {
            query.and_where(
                Expr::col(a("reason")).eq(if reason == FinishReason::Checkmate {
                    1i32
                } else {
                    2
                }),
            );
        }
        if let Some(min) = f.min_ply {
            query.and_where(Expr::col(a("final_move_ply")).gte(min as i64));
        }
        if let Some(max) = f.max_ply {
            query.and_where(Expr::col(a("final_move_ply")).lte(max as i64));
        }
        if let Some(seek) = search.seek() {
            query.cond_where(
                Cond::any()
                    .add(Expr::col(a("finished_at")).lt(seek.finished_at))
                    .add(
                        Cond::all()
                            .add(Expr::col(a("finished_at")).eq(seek.finished_at))
                            .add(
                                Expr::cust("id COLLATE \"C\"")
                                    .lt(crate::encode_id(seek.id.as_str())),
                            ),
                    ),
            );
        }
        query
            .order_by(a("finished_at"), Order::Desc)
            .order_by_expr(Expr::cust("id COLLATE \"C\""), Order::Desc)
            .limit(u64::from(search.limit()) + 1);
        let (sql, values) = query.build_sqlx(PostgresQueryBuilder);
        let mut connection = self.connection().await?;
        let rows = sqlx::query_with(AssertSqlSafe(sql), values)
            .fetch_all(&mut *connection)
            .await
            .map_err(unavailable)?;
        let mut items = rows.iter().map(metadata).collect::<Result<Vec<_>, _>>()?;
        let next_cursor = if items.len() > search.limit() as usize {
            items.pop();
            items.last().map(|last| search.cursor(last))
        } else {
            None
        };
        Ok(Page { items, next_cursor })
    }
    async fn detail(&self, id: &GameId) -> Result<Detail, Error> {
        let mut connection = self.connection().await?;
        let mut tx = sqlx::Connection::begin(&mut *connection)
            .await
            .map_err(unavailable)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        let (sql, values) = metadata_query()
            .and_where(Expr::col(a("id")).eq(crate::encode_id(id.as_str())))
            .build_sqlx(PostgresQueryBuilder);
        let row = sqlx::query_with(AssertSqlSafe(sql), values)
            .fetch_optional(&mut *tx)
            .await
            .map_err(unavailable)?
            .ok_or(Error::NotFound)?;
        let metadata = metadata(&row)?;
        if metadata.final_seq > MAX_HISTORY {
            return Err(Error::HistoryTooLong);
        }
        let (sql, values) = Query::select()
            .columns(
                [
                    "seq", "user_id", "action", "usi", "in_check", "winner", "reason",
                ]
                .map(a),
            )
            .from(a("journal"))
            .and_where(Expr::col(a("game_id")).eq(crate::encode_id(id.as_str())))
            .order_by(a("seq"), Order::Asc)
            .limit(MAX_HISTORY + 1)
            .build_sqlx(PostgresQueryBuilder);
        let rows = sqlx::query_with(AssertSqlSafe(sql), values)
            .fetch_all(&mut *tx)
            .await
            .map_err(unavailable)?;
        if rows.len() as u64 != metadata.final_seq {
            return Err(Error::Corrupt);
        }
        let mut history = Vec::with_capacity(rows.len());
        let mut move_ply = 0;
        for (index, row) in rows.iter().enumerate() {
            let seq = number(row, "seq")?;
            if seq != index as u64 + 1 {
                return Err(Error::Corrupt);
            }
            let user = decode(row.try_get("user_id").map_err(corrupt)?)?;
            let player = if user == metadata.black.as_str() {
                Player::Black
            } else if user == metadata.white.as_str() {
                Player::White
            } else {
                return Err(Error::Corrupt);
            };
            let usi: Option<String> = row.try_get("usi").map_err(corrupt)?;
            let action = match (row.try_get::<i32, _>("action").map_err(corrupt)?, usi) {
                (1, Some(usi)) => {
                    if player
                        != if move_ply % 2 == 0 {
                            Player::Black
                        } else {
                            Player::White
                        }
                    {
                        return Err(Error::Corrupt);
                    }
                    move_ply += 1;
                    PublicAction::Move(usi.parse().map_err(corrupt)?)
                }
                (2, None) => PublicAction::Resign,
                _ => return Err(Error::Corrupt),
            };
            let winner: Option<i32> = row.try_get("winner").map_err(corrupt)?;
            let finish: Option<i32> = row.try_get("reason").map_err(corrupt)?;
            if seq == metadata.final_seq {
                if winner.map(side).transpose()? != Some(metadata.result.winner)
                    || finish.map(reason).transpose()? != Some(metadata.result.reason)
                {
                    return Err(Error::Corrupt);
                }
                match (&action, metadata.result.reason) {
                    (PublicAction::Resign, FinishReason::Resignation)
                        if player.opponent() == metadata.result.winner => {}
                    (PublicAction::Move(_), FinishReason::Checkmate)
                        if player == metadata.result.winner
                            && row.try_get::<bool, _>("in_check").map_err(corrupt)? => {}
                    _ => return Err(Error::Corrupt),
                }
            } else if winner.is_some() || finish.is_some() || matches!(action, PublicAction::Resign)
            {
                return Err(Error::Corrupt);
            }
            history.push(PublicOperation {
                seq,
                player,
                action,
            });
        }
        if move_ply != metadata.final_move_ply {
            return Err(Error::Corrupt);
        }
        tx.commit().await.map_err(unavailable)?;
        Ok(Detail { metadata, history })
    }
}

fn decode(value: String) -> Result<String, Error> {
    crate::decode_id(&value).map_err(corrupt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn database_role_denies_writes_even_when_read_only_mode_is_disabled() {
        let database = pg_test_support::TestDatabase::new().await;
        let writer = crate::SeaOrmStore::connect(database.url()).await.unwrap();
        let reader = PostgresGameRecords::connect(&database.reader_url().await)
            .await
            .unwrap();
        let mut connection = reader.connection().await.unwrap();
        sqlx::query("SET default_transaction_read_only = off")
            .execute(&mut *connection)
            .await
            .unwrap();
        let (sql, values) = Query::delete()
            .from_table(a("games"))
            .build_sqlx(PostgresQueryBuilder);
        let error = sqlx::query_with(AssertSqlSafe(sql), values)
            .execute(&mut *connection)
            .await
            .unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("42501")
        );
        drop(connection);
        reader.close().await;
        writer.close().await.unwrap();
    }

    #[tokio::test]
    async fn statement_timeout_interrupts_real_database_work_and_connection_recovers() {
        let database = pg_test_support::TestDatabase::new().await;
        let writer = crate::SeaOrmStore::connect(database.url()).await.unwrap();
        let reader = PostgresGameRecords::connect(&database.reader_url().await)
            .await
            .unwrap();
        let mut connection = reader.connection().await.unwrap();
        let error = sqlx::query("SELECT pg_sleep(10)")
            .execute(&mut *connection)
            .await
            .unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("57014")
        );
        sqlx::query("SELECT 1")
            .execute(&mut *connection)
            .await
            .unwrap();
        drop(connection);
        reader.close().await;
        writer.close().await.unwrap();
    }
}
