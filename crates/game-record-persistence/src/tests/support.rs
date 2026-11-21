#![cfg(test)]

use super::*;

pub(super) async fn initialize(url: &str) {
    let pool = PgPool::connect(url).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../../game-persistence/tests/fixtures/schema-v9.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
}
