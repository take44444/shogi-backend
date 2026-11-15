crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL timeout settings and timed work cancellation/recovery.",
        reason: "Actual timeout settings are checked, pg_sleep is cancelled with SQLSTATE 57014 and the connection recovers.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
    async fn statement_timeout_interrupts_real_database_work_and_connection_recovers() {
        let database = pg_test_support::TestDatabase::new().await;
        initialize(database.url()).await;
        let reader = PostgresGameRecords::connect(&database.reader_url().await)
            .await
            .unwrap();
        let mut connection = reader.connection().await.unwrap();
        for command in ["SHOW statement_timeout", "SHOW lock_timeout"] {
            let row = sqlx::query(command)
                .fetch_one(&mut *connection)
                .await
                .unwrap();
            assert_eq!(row.get::<String, _>(0), "2s");
        }
        let error = sqlx::query("SELECT pg_sleep(10)")
            .execute(&mut *connection)
            .await
            .unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("57014")
        );
        sqlx::query("SELECT 1")
            .execute(&mut *connection)
            .await
            .unwrap();
        drop(connection);
        reader.close().await;
    }
}
