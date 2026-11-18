#![cfg(test)]

use super::*;

#[path = "public-keysets/tests.rs"]
mod public_record_search_public_keysets;

async fn initialize(url: &str) {
    let pool = PgPool::connect(url).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../../../game-persistence/tests/fixtures/schema-v8.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
}

#[path = "../startup-schema-isolation/reader-permissions/tests.rs"]
mod startup_schema_isolation_reader_permissions;

#[path = "../startup-schema-isolation/reader-boundary/tests.rs"]
mod startup_schema_isolation_reader_boundary;
