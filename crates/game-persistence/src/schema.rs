use sea_orm::sea_query::{
    Alias, ColumnDef, ConditionalStatement, Expr, ExprTrait, ForeignKey, Index, IndexOrder, Query,
    Table, TableCreateStatement,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement, TransactionTrait};

pub(crate) enum SchemaError {
    Database(DbErr),
    Corrupt,
}

impl From<DbErr> for SchemaError {
    fn from(error: DbErr) -> Self {
        Self::Database(error)
    }
}

pub(crate) fn a(name: &str) -> Alias {
    Alias::new(name)
}

pub(crate) async fn initialize(db: &DatabaseConnection) -> Result<(), SchemaError> {
    let tx = db.begin().await?;
    tx.execute_unprepared("SELECT pg_advisory_xact_lock(6001133723959083848)")
        .await?;
    let tables = tx
        .query_all_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT table_name AS name FROM information_schema.tables WHERE table_schema = current_schema() AND table_type = 'BASE TABLE'",
        ))
        .await?;
    if tables.is_empty() {
        for table in [
            Table::create()
                .table(a("schema_version"))
                .col(
                    ColumnDef::new(a("id"))
                        .big_integer()
                        .primary_key()
                        .not_null(),
                )
                .col(ColumnDef::new(a("version")).big_integer().not_null())
                .to_owned(),
            games(),
            journal(),
            outbox(),
        ] {
            tx.execute(&table).await?;
        }
        tx.execute(
            Query::insert()
                .into_table(a("schema_version"))
                .columns([a("id"), a("version")])
                .values_panic([0_i64.into(), 5_i64.into()]),
        )
        .await?;
        tx.execute(
            Index::create()
                .name("outbox_pending")
                .table(a("outbox_events"))
                .col(a("published"))
                .col(a("id")),
        )
        .await?;
        for (name, participant) in [
            ("records_finished", None),
            ("records_black", Some("black")),
            ("records_white", Some("white")),
        ] {
            let mut index = Index::create();
            index.name(name).table(a("games"));
            if let Some(participant) = participant {
                index.col(a(participant));
            }
            index
                .col((a("finished_at"), IndexOrder::Desc))
                .col((Expr::cust("id COLLATE \"C\""), IndexOrder::Desc))
                .and_where(Expr::col(a("visibility")).eq("public"))
                .and_where(Expr::col(a("finished_at")).is_not_null());
            tx.execute(&index).await?;
        }
    } else {
        let names = tables
            .iter()
            .map(|row| row.try_get::<String>("", "name"))
            .collect::<Result<Vec<_>, _>>()?;
        if !["schema_version", "games", "journal", "outbox_events"]
            .iter()
            .all(|name| names.iter().any(|table| table == name))
        {
            return Err(SchemaError::Corrupt);
        }
        let columns = tx
            .query_all_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT column_name AS name FROM information_schema.columns WHERE table_schema = current_schema() AND table_name = 'schema_version'",
            ))
            .await?;
        if !["id", "version"].iter().all(|name| {
            columns.iter().any(|row| {
                row.try_get::<String>("", "name")
                    .is_ok_and(|column| column == *name)
            })
        }) {
            return Err(SchemaError::Corrupt);
        }
        let rows = tx
            .query_all(
                Query::select()
                    .columns([a("id"), a("version")])
                    .from(a("schema_version")),
            )
            .await?;
        if rows.len() != 1
            || rows[0]
                .try_get::<i64>("", "id")
                .map_err(|_| SchemaError::Corrupt)?
                != 0
            || rows[0]
                .try_get::<i64>("", "version")
                .map_err(|_| SchemaError::Corrupt)?
                != 5
        {
            return Err(SchemaError::Corrupt);
        }
    }
    tx.commit().await?;
    Ok(())
}

fn games() -> TableCreateStatement {
    Table::create()
        .table(a("games"))
        .col(
            ColumnDef::new(a("id"))
                .text()
                .extra("COLLATE \"C\"")
                .primary_key()
                .not_null(),
        )
        .col(ColumnDef::new(a("black")).text().not_null())
        .col(ColumnDef::new(a("white")).text().not_null())
        .col(
            ColumnDef::new(a("version"))
                .big_integer()
                .not_null()
                .check(Expr::col(a("version")).gte(0)),
        )
        .col(ColumnDef::new(a("winner")).integer())
        .col(ColumnDef::new(a("reason")).integer())
        .col(
            ColumnDef::new(a("rule"))
                .text()
                .not_null()
                .check(Expr::col(a("rule")).eq("standard")),
        )
        .col(
            ColumnDef::new(a("visibility"))
                .text()
                .not_null()
                .check(Expr::col(a("visibility")).is_in(["public", "private"])),
        )
        .col(
            ColumnDef::new(a("finished_at")).big_integer().check(
                Expr::col(a("finished_at"))
                    .is_null()
                    .or(Expr::col(a("finished_at")).gte(0)),
            ),
        )
        .col(
            ColumnDef::new(a("final_move_ply"))
                .big_integer()
                .not_null()
                .check(Expr::col(a("final_move_ply")).gte(0)),
        )
        .check(Expr::col(a("black")).ne(Expr::col(a("white"))))
        .check(
            Expr::col(a("winner"))
                .is_null()
                .and(Expr::col(a("reason")).is_null())
                .and(Expr::col(a("finished_at")).is_null())
                .or(Expr::col(a("winner"))
                    .is_not_null()
                    .and(Expr::col(a("reason")).is_not_null())
                    .and(Expr::col(a("finished_at")).is_not_null())
                    .and(Expr::col(a("winner")).is_in([1, 2]))
                    .and(Expr::col(a("reason")).is_in([1, 2]))),
        )
        .to_owned()
}

fn journal() -> TableCreateStatement {
    Table::create()
        .table(a("journal"))
        .col(ColumnDef::new(a("game_id")).text().not_null())
        .col(
            ColumnDef::new(a("seq"))
                .big_integer()
                .not_null()
                .check(Expr::col(a("seq")).gt(0)),
        )
        .col(ColumnDef::new(a("request_id")).text().not_null())
        .col(ColumnDef::new(a("user_id")).text().not_null())
        .col(
            ColumnDef::new(a("expected_seq"))
                .big_integer()
                .not_null()
                .check(Expr::col(a("expected_seq")).gte(0)),
        )
        .col(ColumnDef::new(a("action")).integer().not_null())
        .col(ColumnDef::new(a("usi")).text())
        .col(ColumnDef::new(a("in_check")).boolean().not_null())
        .col(ColumnDef::new(a("winner")).integer())
        .col(ColumnDef::new(a("reason")).integer())
        .primary_key(Index::create().col(a("game_id")).col(a("seq")))
        .index(
            Index::create()
                .unique()
                .col(a("game_id"))
                .col(a("request_id")),
        )
        .foreign_key(
            ForeignKey::create()
                .from(a("journal"), a("game_id"))
                .to(a("games"), a("id")),
        )
        .check(Expr::col(a("expected_seq")).eq(Expr::col(a("seq")).sub(1)))
        .check(
            Expr::col(a("action"))
                .eq(1)
                .and(Expr::col(a("usi")).is_not_null())
                .or(Expr::col(a("action"))
                    .eq(2)
                    .and(Expr::col(a("usi")).is_null())),
        )
        .check(result_pair())
        .to_owned()
}

fn outbox() -> TableCreateStatement {
    Table::create()
        .table(a("outbox_events"))
        .col(
            ColumnDef::new(a("id"))
                .big_integer()
                .auto_increment()
                .primary_key(),
        )
        .col(ColumnDef::new(a("game_id")).text().not_null())
        .col(ColumnDef::new(a("seq")).big_integer().not_null())
        .col(
            ColumnDef::new(a("event_kind"))
                .integer()
                .not_null()
                .check(Expr::col(a("event_kind")).is_in([0, 1])),
        )
        .col(ColumnDef::new(a("encoding_version")).integer().not_null())
        .col(ColumnDef::new(a("request_id")).text().not_null())
        .col(ColumnDef::new(a("user_id")).text().not_null())
        .col(ColumnDef::new(a("usi")).text())
        .col(ColumnDef::new(a("in_check")).boolean())
        .col(ColumnDef::new(a("winner")).integer())
        .col(ColumnDef::new(a("reason")).integer())
        .col(
            ColumnDef::new(a("published"))
                .boolean()
                .not_null()
                .default(false),
        )
        .index(
            Index::create()
                .unique()
                .col(a("game_id"))
                .col(a("seq"))
                .col(a("event_kind")),
        )
        .foreign_key(
            ForeignKey::create()
                .from(a("outbox_events"), (a("game_id"), a("seq")))
                .to(a("journal"), (a("game_id"), a("seq"))),
        )
        .check(result_pair())
        .check(
            Expr::col(a("event_kind"))
                .eq(0)
                .and(Expr::col(a("usi")).is_not_null())
                .and(Expr::col(a("in_check")).is_not_null())
                .and(
                    Expr::col(a("reason"))
                        .is_null()
                        .or(Expr::col(a("reason")).eq(1)),
                )
                .or(Expr::col(a("event_kind"))
                    .eq(1)
                    .and(Expr::col(a("usi")).is_null())
                    .and(Expr::col(a("in_check")).is_null())
                    .and(Expr::col(a("winner")).is_not_null())),
        )
        .to_owned()
}

fn result_pair() -> Expr {
    Expr::col(a("winner"))
        .is_null()
        .and(Expr::col(a("reason")).is_null())
        .or(Expr::col(a("winner"))
            .is_not_null()
            .and(Expr::col(a("reason")).is_not_null())
            .and(Expr::col(a("winner")).is_in([1, 2]))
            .and(Expr::col(a("reason")).is_in([1, 2])))
}
