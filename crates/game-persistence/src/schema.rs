//! SQLite schema bootstrap and version validation under an immediate transaction.
//! Tables, constraints, and the version ledger commit together for a new database.

use sea_orm::sea_query::{Alias, ColumnDef, Expr, ExprTrait, Query, Table};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, DbErr, SqliteTransactionMode,
    TransactionOptions, TransactionTrait,
};
use sea_schema::probe::SchemaProbe;

/// Distinguishes database failures from an incompatible or incomplete schema.
pub(crate) enum MigrationError {
    Database(DbErr),
    Corrupt,
}

impl From<DbErr> for MigrationError {
    fn from(error: DbErr) -> Self {
        Self::Database(error)
    }
}

pub(crate) fn a(name: &str) -> Alias {
    Alias::new(name)
}

pub(crate) async fn migrate(db: &DatabaseConnection) -> Result<(), MigrationError> {
    let tx = db
        .begin_with_options(TransactionOptions {
            sqlite_transaction_mode: Some(SqliteTransactionMode::Immediate),
            ..Default::default()
        })
        .await?;
    let query = match tx.get_database_backend() {
        DbBackend::Sqlite => sea_schema::sqlite::Sqlite.query_tables(),
        DbBackend::Postgres => sea_schema::postgres::Postgres.query_tables(),
        DbBackend::MySql => sea_schema::mysql::MySql.query_tables(),
        _ => return Err(DbErr::Custom("unsupported database backend".into()).into()),
    };
    let tables = tx.query_all(&query).await?;
    let names = tables
        .iter()
        .map(|row| row.try_get::<String>("", "table_name"))
        .collect::<Result<Vec<_>, _>>()?;
    let owned = ["users", "legacy_users", "games", "journal", "outbox_events"];
    let exists = |name: &str| names.iter().any(|table| table == name);
    if !exists("schema_version") {
        if owned.iter().any(|table| exists(table)) {
            return Err(MigrationError::Corrupt);
        }
        tx.execute(
            Table::create()
                .table(a("schema_version"))
                .col(ColumnDef::new(a("id")).integer().not_null().primary_key())
                .col(ColumnDef::new(a("version")).integer().not_null()),
        )
        .await?;
        tx.execute(
            Query::insert()
                .into_table(a("schema_version"))
                .columns([a("id"), a("version")])
                .values_panic([0.into(), 0.into()]),
        )
        .await?;
    }
    let ledger_columns = tx
        .query_all_raw(sea_orm::Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA table_info(schema_version)",
        ))
        .await?;
    let has_column = |name: &str| {
        ledger_columns.iter().any(|row| {
            row.try_get::<String>("", "name")
                .is_ok_and(|column| column == name)
        })
    };
    if !has_column("id") || !has_column("version") {
        return Err(MigrationError::Corrupt);
    }
    let rows = tx
        .query_all(
            Query::select()
                .columns([a("id"), a("version")])
                .from(a("schema_version")),
        )
        .await?;
    if rows.len() != 1 {
        return Err(MigrationError::Corrupt);
    }
    let row = &rows[0];
    let id: i32 = row.try_get("", "id").map_err(|_| MigrationError::Corrupt)?;
    let version: i32 = row
        .try_get("", "version")
        .map_err(|_| MigrationError::Corrupt)?;
    if id != 0 {
        return Err(MigrationError::Corrupt);
    }
    match version {
        0 => {
            if owned.iter().any(|table| exists(table)) {
                return Err(MigrationError::Corrupt);
            }
        }
        1 | 2 => {
            if !["users", "games", "journal"]
                .iter()
                .all(|table| exists(table))
                || exists("legacy_users")
                || exists("outbox_events") != (version == 2)
            {
                return Err(MigrationError::Corrupt);
            }
        }
        3 => {
            if !["games", "journal", "outbox_events"]
                .iter()
                .all(|table| exists(table))
            {
                return Err(MigrationError::Corrupt);
            }
        }
        _ => return Err(MigrationError::Corrupt),
    }
    if version < 3 {
        if version > 0 {
            if !tx
                .query_all_raw(sea_orm::Statement::from_string(
                    DbBackend::Sqlite,
                    "PRAGMA foreign_key_check",
                ))
                .await?
                .is_empty()
            {
                return Err(MigrationError::Corrupt);
            }
            // Child-first removal keeps foreign-key enforcement active throughout migration.
            if version == 2 {
                tx.execute_unprepared(
                    "CREATE TEMP TABLE migrate_outbox AS SELECT * FROM outbox_events",
                )
                .await?;
                tx.execute_unprepared("DROP TABLE outbox_events").await?;
            }
            tx.execute_unprepared("CREATE TEMP TABLE migrate_journal AS SELECT * FROM journal")
                .await?;
            tx.execute_unprepared("DROP TABLE journal").await?;
            tx.execute_unprepared("CREATE TEMP TABLE migrate_games AS SELECT * FROM games")
                .await?;
            tx.execute_unprepared("DROP TABLE games").await?;
            tx.execute_unprepared("ALTER TABLE users RENAME TO legacy_users")
                .await?;
        }
        tx.execute_unprepared(GAMES).await?;
        tx.execute_unprepared(JOURNAL).await?;
        tx.execute_unprepared(OUTBOX).await?;
        tx.execute_unprepared("CREATE INDEX outbox_pending ON outbox_events(published, id)")
            .await?;
        if version > 0 {
            tx.execute_unprepared("INSERT INTO games (id, black, white, version, winner, reason) SELECT id, black, white, version, winner, reason FROM migrate_games").await?;
            tx.execute_unprepared("INSERT INTO journal (game_id, seq, request_id, user_id, expected_seq, action, usi, in_check, winner, reason) SELECT game_id, seq, request_id, user_id, expected_seq, action, usi, in_check, winner, reason FROM migrate_journal").await?;
            if version == 2 {
                tx.execute_unprepared("INSERT INTO outbox_events (id, game_id, seq, event_kind, encoding_version, request_id, user_id, usi, in_check, winner, reason, published) SELECT id, game_id, seq, event_kind, encoding_version, request_id, user_id, usi, in_check, winner, reason, published FROM migrate_outbox").await?;
                tx.execute_unprepared("DROP TABLE migrate_outbox").await?;
            }
            tx.execute_unprepared("DROP TABLE migrate_journal").await?;
            tx.execute_unprepared("DROP TABLE migrate_games").await?;
        }
        tx.execute(
            Query::update()
                .table(a("schema_version"))
                .value(a("version"), 3)
                .and_where(Expr::col(a("id")).eq(0)),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

const GAMES: &str = "CREATE TABLE games (
    id TEXT PRIMARY KEY NOT NULL, black TEXT NOT NULL, white TEXT NOT NULL,
    version BIGINT NOT NULL CHECK(version >= 0), winner INTEGER, reason INTEGER,
    CHECK(black != white),
    CHECK((winner IS NULL AND reason IS NULL) OR
        (winner IS NOT NULL AND reason IS NOT NULL AND winner IN (1,2) AND reason IN (1,2)))
)";

const JOURNAL: &str = "CREATE TABLE journal (
    game_id TEXT NOT NULL REFERENCES games(id), seq BIGINT NOT NULL CHECK(seq > 0),
    request_id TEXT NOT NULL, user_id TEXT NOT NULL,
    expected_seq BIGINT NOT NULL CHECK(expected_seq >= 0), action INTEGER NOT NULL,
    usi TEXT, in_check BOOLEAN NOT NULL, winner INTEGER, reason INTEGER,
    PRIMARY KEY(game_id, seq), UNIQUE(game_id, request_id),
    CHECK(expected_seq = seq - 1),
    CHECK((action = 1 AND usi IS NOT NULL) OR (action = 2 AND usi IS NULL)),
    CHECK((winner IS NULL AND reason IS NULL) OR
        (winner IS NOT NULL AND reason IS NOT NULL AND winner IN (1,2) AND reason IN (1,2)))
)";

const OUTBOX: &str = "CREATE TABLE outbox_events (
    id INTEGER PRIMARY KEY, game_id TEXT NOT NULL, seq BIGINT NOT NULL,
    event_kind INTEGER NOT NULL CHECK(event_kind IN (0,1)), encoding_version INTEGER NOT NULL,
    request_id TEXT NOT NULL, user_id TEXT NOT NULL, usi TEXT, in_check BOOLEAN,
    winner INTEGER, reason INTEGER,
    published BOOLEAN NOT NULL DEFAULT 0 CHECK(published IN (0,1)),
    UNIQUE(game_id, seq, event_kind),
    FOREIGN KEY(game_id, seq) REFERENCES journal(game_id, seq),
    CHECK((winner IS NULL AND reason IS NULL) OR
        (winner IS NOT NULL AND reason IS NOT NULL AND winner IN (1,2) AND reason IN (1,2))),
    CHECK((event_kind = 0 AND usi IS NOT NULL AND in_check IS NOT NULL
            AND in_check IN (0,1) AND (reason IS NULL OR reason = 1))
        OR (event_kind = 1 AND usi IS NULL AND in_check IS NULL AND winner IS NOT NULL))
)";
