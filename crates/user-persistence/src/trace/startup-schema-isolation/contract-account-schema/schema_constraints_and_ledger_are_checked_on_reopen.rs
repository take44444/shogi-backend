#[tokio::test]
    async fn schema_constraints_and_ledger_are_checked_on_reopen() {
        let database = pg_test_support::TestDatabase::new().await;
        let url = database.url();
        let store = PostgresAccounts::connect(url).await.unwrap();
        store
            .db
            .execute_unprepared("DROP TABLE external_identities")
            .await
            .unwrap();
        store
            .db
            .execute_unprepared(
                "CREATE TABLE external_identities (issuer TEXT, subject TEXT, user_id TEXT)",
            )
            .await
            .unwrap();
        store.close().await.unwrap();
        assert!(matches!(
            PostgresAccounts::connect(url).await,
            Err(Error::Corrupt)
        ));
    }
