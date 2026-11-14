use crate::schema::a;
use sea_orm::sea_query::{
    ColumnDef, ConditionalStatement, Expr, ExprTrait, ForeignKey, Index, Table,
};
use sea_orm::{ConnectionTrait, DatabaseTransaction, DbErr};

pub(crate) async fn create(tx: &DatabaseTransaction) -> Result<(), DbErr> {
    let mut table = Table::create();
    table.table(a("matchmaking_tickets"));
    for name in [
        "user_id",
        "ticket_id",
        "pool",
        "state",
        "game_id",
        "refusal_reason",
    ] {
        let mut column = ColumnDef::new(a(name));
        column.text();
        if matches!(name, "user_id" | "ticket_id" | "state") {
            column.not_null();
        }
        table.col(&mut column);
    }
    for name in [
        "enqueued_at",
        "expires_at",
        "rating",
        "completed",
        "heartbeat_seq",
    ] {
        table.col(ColumnDef::new(a(name)).big_integer());
    }
    table
        .col(ColumnDef::new(a("side")).integer())
        .primary_key(Index::create().col(a("user_id")).col(a("ticket_id")))
        .index(Index::create().unique().col(a("game_id")).col(a("side")))
        .foreign_key(
            ForeignKey::create()
                .from(a("matchmaking_tickets"), a("game_id"))
                .to(a("games"), a("id")),
        )
        .check(
            Expr::col(a("pool"))
                .is_null()
                .or(Expr::col(a("pool")).is_in(["ten_minute_30", "byoyomi_30"])),
        )
        .check(
            Expr::col(a("pool"))
                .is_not_null()
                .or(Expr::col(a("state")).eq("cancelled")),
        )
        .check(Expr::col(a("state")).is_in([
            "waiting",
            "playing",
            "finished",
            "cancelled",
            "expired",
            "refused",
        ]))
        .check(
            Expr::col(a("enqueued_at"))
                .is_null()
                .and(Expr::col(a("expires_at")).is_null())
                .and(Expr::col(a("rating")).is_null())
                .and(Expr::col(a("completed")).is_null())
                .and(Expr::col(a("heartbeat_seq")).is_null())
                .or(Expr::col(a("enqueued_at"))
                    .is_not_null()
                    .and(Expr::col(a("expires_at")).is_not_null())
                    .and(Expr::col(a("expires_at")).gt(Expr::col(a("enqueued_at"))))
                    .and(Expr::col(a("completed")).is_not_null())
                    .and(Expr::col(a("heartbeat_seq")).is_not_null())
                    .and(Expr::col(a("heartbeat_seq")).gte(0))
                    .and(
                        Expr::col(a("rating"))
                            .is_null()
                            .and(Expr::col(a("completed")).eq(0))
                            .or(Expr::col(a("rating"))
                                .is_not_null()
                                .and(Expr::col(a("completed")).gt(0))),
                    )),
        )
        .check(
            Expr::col(a("state"))
                .ne("waiting")
                .or(Expr::col(a("enqueued_at"))
                    .is_not_null()
                    .and(Expr::col(a("game_id")).is_null())),
        )
        .check(
            Expr::col(a("state"))
                .is_in(["playing", "finished"])
                .and(Expr::col(a("game_id")).is_not_null())
                .and(Expr::col(a("side")).is_not_null())
                .and(Expr::col(a("side")).is_in([1, 2]))
                .and(Expr::col(a("enqueued_at")).is_not_null())
                .or(Expr::col(a("state"))
                    .is_not_in(["playing", "finished"])
                    .and(Expr::col(a("game_id")).is_null())
                    .and(Expr::col(a("side")).is_null())),
        )
        .check(
            Expr::col(a("state"))
                .eq("refused")
                .and(Expr::col(a("refusal_reason")).is_not_null())
                .and(Expr::col(a("refusal_reason")).is_in([
                    "already_waiting",
                    "already_playing",
                    "rating_pending",
                ]))
                .and(Expr::col(a("enqueued_at")).is_null())
                .or(Expr::col(a("state"))
                    .ne("refused")
                    .and(Expr::col(a("refusal_reason")).is_null())),
        );
    tx.execute(&table).await?;
    tx.execute(
        Index::create()
            .unique()
            .name("matchmaking_active_user")
            .table(a("matchmaking_tickets"))
            .col(a("user_id"))
            .and_where(Expr::col(a("state")).is_in(["waiting", "playing"])),
    )
    .await?;
    for (name, column) in [
        ("matchmaking_waiting_order", "enqueued_at"),
        ("matchmaking_waiting_rating", "rating"),
    ] {
        tx.execute(
            Index::create()
                .name(name)
                .table(a("matchmaking_tickets"))
                .col(a("pool"))
                .col(a(column))
                .col(a("ticket_id"))
                .and_where(Expr::col(a("state")).eq("waiting")),
        )
        .await?;
    }
    Ok(())
}
