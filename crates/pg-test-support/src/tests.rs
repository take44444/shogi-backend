use super::*;
use sqlx::Row;

#[tokio::test]
async fn test_databases_have_independent_names_and_contents() {
    let first = TestDatabase::new().await;
    let second = TestDatabase::new().await;
    assert_ne!(first.url(), second.url());
    let mut left = PgConnection::connect(first.url()).await.unwrap();
    sqlx::raw_sql(
        "CREATE TABLE isolation_probe (id INTEGER); INSERT INTO isolation_probe VALUES (1)",
    )
    .execute(&mut left)
    .await
    .unwrap();
    let row = sqlx::query("SELECT count(*) AS n FROM isolation_probe")
        .fetch_one(&mut left)
        .await
        .unwrap();
    assert_eq!(row.get::<i64, _>("n"), 1);
    let mut right = PgConnection::connect(second.url()).await.unwrap();
    let row = sqlx::query("SELECT to_regclass('public.isolation_probe')::text AS name")
        .fetch_one(&mut right)
        .await
        .unwrap();
    assert_eq!(row.get::<Option<String>, _>("name"), None);
}
