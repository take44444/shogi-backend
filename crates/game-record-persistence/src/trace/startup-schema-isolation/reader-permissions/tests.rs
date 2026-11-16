use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real SELECT-only PostgreSQL role after disabling read-only session default.",
        reason: "The actual reader role receives insufficient_privilege after disabling default_transaction_read_only and attempting a write.",
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
