use crate::{PostgresGameRecords, a, corrupt, encode_id, unavailable};
use async_trait::async_trait;
use game_record_application::ratings::{Pool, RatingRepository, RatingState, UserRatingSnapshot};
use game_record_application::{Error, UserId};
use sea_orm::sea_query::{Expr, ExprTrait, PostgresQueryBuilder, Query};
use sea_orm::sqlx::{self, AssertSqlSafe, Row};
use sea_query_sqlx::SqlxBinder;

#[async_trait]
impl RatingRepository for PostgresGameRecords {
    async fn ratings(&self, user: &UserId) -> Result<UserRatingSnapshot, Error> {
        let (sql, values) = Query::select()
            .columns([a("pool"), a("rating"), a("completed")])
            .from(a("rating_states"))
            .and_where(Expr::col(a("user_id")).eq(encode_id(user.as_str())))
            .build_sqlx(PostgresQueryBuilder);
        let mut connection = self.connection().await?;
        let rows = sqlx::query_with(AssertSqlSafe(sql), values)
            .fetch_all(&mut *connection)
            .await
            .map_err(unavailable)?;
        let mut snapshot = UserRatingSnapshot::default();
        for row in rows {
            let state = RatingState::from_parts(
                row.try_get("rating").map_err(corrupt)?,
                row.try_get("completed").map_err(corrupt)?,
            )
            .map_err(corrupt)?;
            match Pool::parse(&row.try_get::<String, _>("pool").map_err(corrupt)?)
                .map_err(corrupt)?
            {
                Pool::TenMinute30 => snapshot.ten_minute_30 = state,
                Pool::Byoyomi30 => snapshot.byoyomi_30 = state,
            }
        }
        Ok(snapshot)
    }
}
