use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実 PostgreSQL の読み取り接続プールを使い切った状態で、接続取得と復旧を確認する。",
        reason: "四つの接続を保持すると、五つ目の取得はタイムアウトするまで待機する。接続を解放すると再び取得できることを確認する。",
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
        scope: "実 PostgreSQL のタイムアウト設定と、時間を要する処理の中断・復旧を確認する。",
        reason: "実際のタイムアウト設定を確認し、pg_sleep が SQLSTATE 57014 で中断され、その接続が復旧することを確認する。",
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
