crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "既存 schema fixture と実 PostgreSQL でプロフィール再利用と再接続後の認証を確認する。HTTP と provider は通らない。",
        reason: "既存 schema の Existing プロフィールに対して initial_name が Player の login を行い、同じプロフィールと再接続後の認証結果を確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "startup-schema-isolation", contract: "contract-account-schema", scope: "Real PostgreSQL version-one fixture, existing identity/session, connect/close/reopen.", relationship: crate::trace_support::Relationship::Asserts, reason: "A preexisting version-one fixture and identity/session survive connect, login, close and reopen." }],
    };
#[tokio::test]
    async fn existing_schema_and_data_survive_builder_refactor() {
        let database = pg_test_support::TestDatabase::new().await;
        let db = Database::connect(database.url()).await.unwrap();
        db.execute_unprepared(include_str!("../../../../tests/fixtures/schema-v1.sql"))
            .await
            .unwrap();
        db.execute_unprepared("INSERT INTO accounts VALUES ('existing-user', 'Existing', 100); INSERT INTO external_identities VALUES ('https://accounts.google.com', 'subject', 'existing-user'); INSERT INTO sessions VALUES ('existing-session', 'existing-user', 200)").await.unwrap();
        let store = PostgresAccounts::connect(database.url()).await.unwrap();
        let profile = store.authenticate("existing-session", 101).await.unwrap();
        assert_eq!(profile.id.as_str(), "existing-user");
        assert_eq!(profile.display_name.as_str(), "Existing");
        assert_eq!(
            store
                .login(identity(), session("new-session", 200), 102)
                .await
                .unwrap(),
            profile
        );
        store.close().await.unwrap();
        let reopened = PostgresAccounts::connect(database.url()).await.unwrap();
        assert_eq!(
            reopened.authenticate("new-session", 103).await.unwrap(),
            profile
        );
    }
}
