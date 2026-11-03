//! Public record queries use an independently opened, read-only SQLite pool.
use async_trait::async_trait;
use game_record_application::{
    Detail, Error, GameId, MAX_HISTORY, Metadata, Page, PublicAction, PublicOperation,
    RecordRepository, Rule, Search, UserId,
};
use sea_orm::sqlx::{
    self, Row, Sqlite, SqlitePool,
    pool::PoolConnection,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow},
};
use shogi_domain::{FinishReason, GameResult, Player};
use std::{
    path::Path,
    time::{Duration, Instant},
};

const ELIGIBLE: &str = "visibility = 'public' AND finished_at IS NOT NULL";
const COLUMNS: &str =
    "id, black, white, version, winner, reason, rule, finished_at, final_move_ply";

/// No create, initialization, journal-mode changes, or game mutation capability.
pub struct SqliteGameRecords {
    pool: SqlitePool,
}
impl SqliteGameRecords {
    /// Opens an existing schema-v5 file with SQLite read-only flags and query-only connections.
    /// The game writer must initialize the database first. WAL and SHM files must be readable.
    ///
    /// # Errors
    /// Returns `Unavailable` for I/O or pool failures and `Corrupt` for an unsupported schema.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = std::fs::canonicalize(path).map_err(|_| Error::Unavailable)?;
        if !path.is_file() {
            return Err(Error::Unavailable);
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .read_only(true)
            .create_if_missing(false)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(2));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(2))
            .after_connect(|connection, _| {
                Box::pin(async move {
                    sqlx::query("PRAGMA query_only = ON")
                        .execute(connection)
                        .await?;
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
    async fn connection(&self) -> Result<PoolConnection<Sqlite>, Error> {
        let mut connection = self.pool.acquire().await.map_err(unavailable)?;
        let deadline = Instant::now() + Duration::from_secs(2);
        connection
            .lock_handle()
            .await
            .map_err(unavailable)?
            .set_progress_handler(1000, move || Instant::now() < deadline);
        Ok(connection)
    }
}
fn unavailable(_: impl std::fmt::Debug) -> Error {
    Error::Unavailable
}
fn corrupt(_: impl std::fmt::Debug) -> Error {
    Error::Corrupt
}
async fn validate_schema(connection: &mut PoolConnection<Sqlite>) -> Result<(), Error> {
    let rows = sqlx::query("SELECT id, version FROM schema_version")
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
        let rows = sqlx::query("SELECT * FROM pragma_table_info(?)")
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
                            .try_get::<i64, _>("notnull")
                            .is_ok_and(|value| value == 1)
                })
            })
        {
            return Err(Error::Corrupt);
        }
        let kind = sqlx::query("SELECT type FROM sqlite_master WHERE name = ?")
            .bind(table)
            .fetch_one(&mut **connection)
            .await
            .map_err(corrupt)?;
        if kind.try_get::<String, _>("type").map_err(corrupt)? != "table" {
            return Err(Error::Corrupt);
        }
    }
    Ok(())
}
fn side(value: i64) -> Result<Player, Error> {
    match value {
        1 => Ok(Player::Black),
        2 => Ok(Player::White),
        _ => Err(Error::Corrupt),
    }
}
fn reason(value: i64) -> Result<FinishReason, Error> {
    match value {
        1 => Ok(FinishReason::Checkmate),
        2 => Ok(FinishReason::Resignation),
        _ => Err(Error::Corrupt),
    }
}
fn number(row: &SqliteRow, column: &str) -> Result<u64, Error> {
    u64::try_from(row.try_get::<i64, _>(column).map_err(corrupt)?).map_err(corrupt)
}
fn metadata(row: &SqliteRow) -> Result<Metadata, Error> {
    let data = Metadata {
        id: GameId::new(row.try_get::<String, _>("id").map_err(corrupt)?).map_err(corrupt)?,
        black: UserId::new(row.try_get::<String, _>("black").map_err(corrupt)?).map_err(corrupt)?,
        white: UserId::new(row.try_get::<String, _>("white").map_err(corrupt)?).map_err(corrupt)?,
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
impl RecordRepository for SqliteGameRecords {
    async fn search(&self, search: &Search) -> Result<Page, Error> {
        let f = search.filters();
        let mut query = sqlx::QueryBuilder::<Sqlite>::new(format!(
            "SELECT {COLUMNS} FROM games WHERE {ELIGIBLE} AND finished_at >= "
        ));
        query
            .push_bind(f.from)
            .push(" AND finished_at < ")
            .push_bind(f.to);
        if let Some(id) = &f.participant {
            query
                .push(" AND (black = ")
                .push_bind(id.as_str())
                .push(" OR white = ")
                .push_bind(id.as_str())
                .push(")");
        }
        if let Some(winner) = f.winner {
            query
                .push(" AND winner = ")
                .push_bind(if winner == Player::Black { 1i64 } else { 2 });
        }
        if let Some(reason) = f.reason {
            query
                .push(" AND reason = ")
                .push_bind(if reason == FinishReason::Checkmate {
                    1i64
                } else {
                    2
                });
        }
        if let Some(min) = f.min_ply {
            query.push(" AND final_move_ply >= ").push_bind(min as i64);
        }
        if let Some(max) = f.max_ply {
            query.push(" AND final_move_ply <= ").push_bind(max as i64);
        }
        if let Some(seek) = search.seek() {
            query
                .push(" AND (finished_at < ")
                .push_bind(seek.finished_at)
                .push(" OR (finished_at = ")
                .push_bind(seek.finished_at)
                .push(" AND id COLLATE BINARY < ")
                .push_bind(seek.id.as_str())
                .push("))");
        }
        query
            .push(" ORDER BY finished_at DESC, id COLLATE BINARY DESC LIMIT ")
            .push_bind(i64::from(search.limit()) + 1);
        let mut connection = self.connection().await?;
        let rows = query
            .build()
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
        let mut query = sqlx::QueryBuilder::<Sqlite>::new("SELECT ");
        query
            .push(COLUMNS)
            .push(" FROM games WHERE ")
            .push(ELIGIBLE)
            .push(" AND id = ")
            .push_bind(id.as_str());
        let row = query
            .build()
            .fetch_optional(&mut *tx)
            .await
            .map_err(unavailable)?
            .ok_or(Error::NotFound)?;
        let metadata = metadata(&row)?;
        if metadata.final_seq > MAX_HISTORY {
            return Err(Error::HistoryTooLong);
        }
        let rows = sqlx::query("SELECT seq, user_id, action, usi, in_check, winner, reason FROM journal WHERE game_id = ? ORDER BY seq LIMIT ?")
            .bind(id.as_str()).bind((MAX_HISTORY+1) as i64).fetch_all(&mut *tx).await.map_err(unavailable)?;
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
            let user: String = row.try_get("user_id").map_err(corrupt)?;
            let player = if user == metadata.black.as_str() {
                Player::Black
            } else if user == metadata.white.as_str() {
                Player::White
            } else {
                return Err(Error::Corrupt);
            };
            let usi: Option<String> = row.try_get("usi").map_err(corrupt)?;
            let action = match (row.try_get::<i64, _>("action").map_err(corrupt)?, usi) {
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
            let winner: Option<i64> = row.try_get("winner").map_err(corrupt)?;
            let finish: Option<i64> = row.try_get("reason").map_err(corrupt)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn sqlite_denies_writes_even_with_query_only_disabled() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("game.db");
        let writer = crate::SeaOrmStore::connect(&format!("sqlite://{}?mode=rwc", path.display()))
            .await
            .unwrap();
        let reader = SqliteGameRecords::open(&path).await.unwrap();
        let mut connection = reader.connection().await.unwrap();
        sqlx::query("PRAGMA query_only = OFF")
            .execute(&mut *connection)
            .await
            .unwrap();
        assert!(
            sqlx::query("DELETE FROM games")
                .execute(&mut *connection)
                .await
                .is_err()
        );
        drop(connection);
        reader.close().await;
        writer.close().await.unwrap();
    }
    #[tokio::test]
    async fn progress_handler_interrupts_real_database_work() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("game.db");
        let writer = crate::SeaOrmStore::connect(&format!("sqlite://{}?mode=rwc", path.display()))
            .await
            .unwrap();
        let reader = SqliteGameRecords::open(&path).await.unwrap();
        let mut connection = reader.connection().await.unwrap();
        connection
            .lock_handle()
            .await
            .unwrap()
            .set_progress_handler(1, || false);
        assert!(sqlx::query("WITH RECURSIVE counter(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM counter WHERE n < 100000000) SELECT sum(n) FROM counter").fetch_one(&mut *connection).await.is_err());
        drop(connection);
        assert!(reader.connection().await.is_ok());
        reader.close().await;
        writer.close().await.unwrap();
    }
}
