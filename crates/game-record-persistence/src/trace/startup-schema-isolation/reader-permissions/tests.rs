use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "読み取り専用セッションの既定値を無効にした、SELECT だけを許可する実 PostgreSQL ロール。",
        reason: "default_transaction_read_only を無効にして書き込みを試みると、実際の読み取りロールが insufficient_privilege を受け取ることを確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
    async fn database_role_denies_writes_even_when_read_only_mode_is_disabled() {
        let database = pg_test_support::TestDatabase::new().await;
        initialize(database.url()).await;
        let reader = PostgresGameRecords::connect(&database.reader_url().await)
            .await
            .unwrap();
        let mut connection = reader.connection().await.unwrap();
        sqlx::query("SET default_transaction_read_only = off")
            .execute(&mut *connection)
            .await
            .unwrap();
        let (sql, values) = Query::delete()
            .from_table(a("games"))
            .build_sqlx(PostgresQueryBuilder);
        let error = sqlx::query_with(AssertSqlSafe(sql), values)
            .execute(&mut *connection)
            .await
            .unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("42501")
        );
        drop(connection);
        reader.close().await;
    }
}
