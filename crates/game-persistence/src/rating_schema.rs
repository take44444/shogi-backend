use crate::schema::a;
use sea_orm::sea_query::{
    ColumnDef, Expr, ExprTrait, ForeignKey, Index, Table, TableCreateStatement,
};

fn pool() -> ColumnDef {
    ColumnDef::new(a("pool"))
        .text()
        .not_null()
        .check(Expr::col(a("pool")).is_in(["ten_minute_30", "byoyomi_30"]))
        .to_owned()
}
fn state_pair(rating: &str, count: &str) -> Expr {
    Expr::col(a(rating))
        .is_null()
        .and(Expr::col(a(count)).eq(0))
        .or(Expr::col(a(rating))
            .is_not_null()
            .and(Expr::col(a(count)).gt(0)))
}
pub(crate) fn tables() -> Vec<TableCreateStatement> {
    vec![
        Table::create()
            .table(a("rating_states"))
            .col(ColumnDef::new(a("user_id")).text().not_null())
            .col(pool())
            .col(ColumnDef::new(a("rating")).big_integer())
            .col(ColumnDef::new(a("completed")).big_integer().not_null())
            .primary_key(Index::create().col(a("user_id")).col(a("pool")))
            .check(state_pair("rating", "completed"))
            .to_owned(),
        Table::create()
            .table(a("rated_games"))
            .col(ColumnDef::new(a("game_id")).text().not_null().primary_key())
            .col(pool())
            .col(ColumnDef::new(a("black_start_rating")).big_integer())
            .col(
                ColumnDef::new(a("black_start_count"))
                    .big_integer()
                    .not_null(),
            )
            .col(ColumnDef::new(a("white_start_rating")).big_integer())
            .col(
                ColumnDef::new(a("white_start_count"))
                    .big_integer()
                    .not_null(),
            )
            .check(state_pair("black_start_rating", "black_start_count"))
            .check(state_pair("white_start_rating", "white_start_count"))
            .foreign_key(
                ForeignKey::create()
                    .from(a("rated_games"), a("game_id"))
                    .to(a("games"), a("id")),
            )
            .to_owned(),
        Table::create()
            .table(a("rating_slots"))
            .col(ColumnDef::new(a("user_id")).text().not_null())
            .col(pool())
            .col(ColumnDef::new(a("game_id")).text().not_null())
            .primary_key(Index::create().col(a("user_id")).col(a("pool")))
            .foreign_key(
                ForeignKey::create()
                    .from(a("rating_slots"), (a("user_id"), a("pool")))
                    .to(a("rating_states"), (a("user_id"), a("pool"))),
            )
            .foreign_key(
                ForeignKey::create()
                    .from(a("rating_slots"), a("game_id"))
                    .to(a("rated_games"), a("game_id")),
            )
            .to_owned(),
        Table::create()
            .table(a("rating_jobs"))
            .col(ColumnDef::new(a("game_id")).text().not_null().primary_key())
            .col(
                ColumnDef::new(a("state"))
                    .text()
                    .not_null()
                    .check(Expr::col(a("state")).is_in([
                        "pending",
                        "claimed",
                        "quarantined",
                        "completed",
                    ])),
            )
            .col(ColumnDef::new(a("owner")).text())
            .col(
                ColumnDef::new(a("generation"))
                    .big_integer()
                    .not_null()
                    .check(Expr::col(a("generation")).gte(0)),
            )
            .col(ColumnDef::new(a("lease_until")).big_integer())
            .col(ColumnDef::new(a("available_at")).big_integer().not_null())
            .col(
                ColumnDef::new(a("attempts"))
                    .big_integer()
                    .not_null()
                    .check(Expr::col(a("attempts")).gte(0)),
            )
            .col(ColumnDef::new(a("last_error")).text())
            .check(
                Expr::col(a("state"))
                    .eq("claimed")
                    .and(Expr::col(a("owner")).is_not_null())
                    .and(Expr::col(a("lease_until")).is_not_null())
                    .or(Expr::col(a("state"))
                        .ne("claimed")
                        .and(Expr::col(a("owner")).is_null())
                        .and(Expr::col(a("lease_until")).is_null())),
            )
            .foreign_key(
                ForeignKey::create()
                    .from(a("rating_jobs"), a("game_id"))
                    .to(a("rated_games"), a("game_id")),
            )
            .to_owned(),
        Table::create()
            .table(a("rating_applications"))
            .col(ColumnDef::new(a("game_id")).text().not_null().primary_key())
            .col(pool())
            .col(
                ColumnDef::new(a("final_seq"))
                    .big_integer()
                    .not_null()
                    .check(Expr::col(a("final_seq")).gt(0)),
            )
            .col(
                ColumnDef::new(a("delta_black"))
                    .big_integer()
                    .not_null()
                    .check(Expr::col(a("delta_black")).between(-32_i64, 32_i64)),
            )
            .col(ColumnDef::new(a("black_before_rating")).big_integer())
            .col(
                ColumnDef::new(a("black_before_count"))
                    .big_integer()
                    .not_null(),
            )
            .col(ColumnDef::new(a("white_before_rating")).big_integer())
            .col(
                ColumnDef::new(a("white_before_count"))
                    .big_integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(a("black_after_rating"))
                    .big_integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(a("black_after_count"))
                    .big_integer()
                    .not_null()
                    .check(Expr::col(a("black_after_count")).gt(0)),
            )
            .col(
                ColumnDef::new(a("white_after_rating"))
                    .big_integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(a("white_after_count"))
                    .big_integer()
                    .not_null()
                    .check(Expr::col(a("white_after_count")).gt(0)),
            )
            .check(state_pair("black_before_rating", "black_before_count"))
            .check(state_pair("white_before_rating", "white_before_count"))
            .foreign_key(
                ForeignKey::create()
                    .from(a("rating_applications"), a("game_id"))
                    .to(a("rating_jobs"), a("game_id")),
            )
            .to_owned(),
    ]
}
