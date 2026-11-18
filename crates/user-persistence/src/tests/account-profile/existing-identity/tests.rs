use super::*;

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
