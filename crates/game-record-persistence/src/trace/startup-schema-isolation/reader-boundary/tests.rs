use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL reader pool acquisition saturation and recovery.",
        reason: "Four held connections prevent a fifth until acquisition timeout; releasing a connection restores acquisition.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
    async fn fifth_reader_connection_times_out_until_one_of_four_is_released() {
        let database = pg_test_support::TestDatabase::new().await;
        initialize(database.url()).await;
        let reader = PostgresGameRecords::connect(&database.reader_url().await)
            .await
            .unwrap();
        let mut held = Vec::new();
        for _ in 0..4 {
            held.push(reader.connection().await.unwrap());
        }
        assert_eq!(reader.pool.size(), 4);
        assert_eq!(
            reader.pool.options().get_acquire_timeout(),
            Duration::from_secs(2)
        );
        assert!(matches!(
            reader.pool.acquire().await,
            Err(sqlx::Error::PoolTimedOut)
        ));
        drop(held.pop());
        let mut recovered = reader.connection().await.unwrap();
        sqlx::query("SELECT 1")
            .execute(&mut *recovered)
            .await
            .unwrap();
        drop(recovered);
        drop(held);
        reader.close().await;
    }
}

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
