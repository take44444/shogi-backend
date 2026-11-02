//! SQLite schema bootstrap and version validation under an immediate transaction.
//! Tables, constraints, and the version ledger commit together for a new database.

use sea_orm::sea_query::{Alias, ColumnDef, Expr, ExprTrait, ForeignKey, Index, Query, Table};
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
    let owned = ["users", "games", "journal", "outbox_events"];
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
            tx.execute(
                Table::create().table(a("users")).col(
                    ColumnDef::new(a("id"))
                        .string_len(128)
                        .not_null()
                        .primary_key(),
                ),
            )
            .await?;
            tx.execute(Table::create().table(a("games"))
                .col(ColumnDef::new(a("id")).string_len(128).not_null().primary_key())
                .col(ColumnDef::new(a("black")).string_len(128).not_null())
                .col(ColumnDef::new(a("white")).string_len(128).not_null())
                .col(ColumnDef::new(a("version")).big_integer().not_null())
                .col(ColumnDef::new(a("winner")).integer())
                .col(ColumnDef::new(a("reason")).integer())
                .check(Expr::col(a("black")).ne(Expr::col(a("white"))))
                .check(Expr::col(a("version")).gte(0))
                .check(Expr::cust("(winner IS NULL AND reason IS NULL) OR (winner IS NOT NULL AND reason IS NOT NULL AND winner IN (1, 2) AND reason IN (1, 2))"))
                .foreign_key(ForeignKey::create().from(a("games"), a("black")).to(a("users"), a("id")))
                .foreign_key(ForeignKey::create().from(a("games"), a("white")).to(a("users"), a("id")))).await?;
            tx.execute(Table::create().table(a("journal"))
                .col(ColumnDef::new(a("game_id")).string_len(128).not_null())
                .col(ColumnDef::new(a("seq")).big_integer().not_null())
                .col(ColumnDef::new(a("request_id")).string_len(128).not_null())
                .col(ColumnDef::new(a("user_id")).string_len(128).not_null())
                .col(ColumnDef::new(a("expected_seq")).big_integer().not_null())
                .col(ColumnDef::new(a("action")).integer().not_null())
                .col(ColumnDef::new(a("usi")).string_len(5))
                .col(ColumnDef::new(a("in_check")).boolean().not_null())
                .col(ColumnDef::new(a("winner")).integer())
                .col(ColumnDef::new(a("reason")).integer())
                .primary_key(Index::create().col(a("game_id")).col(a("seq")))
                .index(Index::create().unique().col(a("game_id")).col(a("request_id")))
                .check(Expr::col(a("seq")).gt(0))
                .check(Expr::col(a("expected_seq")).gte(0))
                .check(Expr::cust("expected_seq = seq - 1"))
                .check(Expr::cust("(action = 1 AND usi IS NOT NULL) OR (action = 2 AND usi IS NULL)"))
                .check(Expr::cust("(winner IS NULL AND reason IS NULL) OR (winner IS NOT NULL AND reason IS NOT NULL AND winner IN (1, 2) AND reason IN (1, 2))"))
                .foreign_key(ForeignKey::create().from(a("journal"), a("game_id")).to(a("games"), a("id")))
                .foreign_key(ForeignKey::create().from(a("journal"), a("user_id")).to(a("users"), a("id")))).await?;
        }
        1 => {
            if !["users", "games", "journal"]
                .iter()
                .all(|table| exists(table))
                || exists("outbox_events")
            {
                return Err(MigrationError::Corrupt);
            }
        }
        2 => {
            if !owned.iter().all(|table| exists(table)) {
                return Err(MigrationError::Corrupt);
            }
        }
        _ => return Err(MigrationError::Corrupt),
    }
    if version < 2 {
        tx.execute_unprepared("CREATE TABLE outbox_events (
            id INTEGER PRIMARY KEY, game_id TEXT NOT NULL, seq BIGINT NOT NULL,
            event_kind INTEGER NOT NULL CHECK(event_kind IN (0, 1)),
            encoding_version INTEGER NOT NULL, request_id TEXT NOT NULL, user_id TEXT NOT NULL,
            usi TEXT, in_check BOOLEAN, winner INTEGER, reason INTEGER,
            published BOOLEAN NOT NULL DEFAULT 0 CHECK(published IN (0, 1)),
            UNIQUE(game_id, seq, event_kind),
            FOREIGN KEY(game_id, seq) REFERENCES journal(game_id, seq),
            CHECK((winner IS NULL AND reason IS NULL) OR
                (winner IS NOT NULL AND reason IS NOT NULL AND winner IN (1,2) AND reason IN (1,2))),
            CHECK((event_kind = 0 AND usi IS NOT NULL AND in_check IS NOT NULL
                    AND in_check IN (0,1) AND (reason IS NULL OR reason = 1))
                OR (event_kind = 1 AND usi IS NULL AND in_check IS NULL AND winner IS NOT NULL))
        )").await?;
        tx.execute_unprepared("CREATE INDEX outbox_pending ON outbox_events(published, id)")
            .await?;
        tx.execute(
            Query::update()
                .table(a("schema_version"))
                .value(a("version"), 2)
                .and_where(Expr::col(a("id")).eq(0)),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
