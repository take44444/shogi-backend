use sea_orm::sea_query::{Alias, ColumnDef, Expr, ExprTrait, ForeignKey, Index, Query, Table};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, DbErr, SqliteTransactionMode,
    TransactionOptions, TransactionTrait,
};
use sea_schema::probe::SchemaProbe;

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
    let owned = ["users", "games", "journal"];
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
            tx.execute(
                Query::update()
                    .table(a("schema_version"))
                    .value(a("version"), 1)
                    .and_where(Expr::col(a("id")).eq(0)),
            )
            .await?;
        }
        1 => {
            if !owned.iter().all(|table| exists(table)) {
                return Err(MigrationError::Corrupt);
            }
        }
        _ => return Err(MigrationError::Corrupt),
    }
    tx.commit().await?;
    Ok(())
}
