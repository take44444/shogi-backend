#[tokio::test]
async fn failed_bootstrap_rolls_back_the_entire_schema() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let raw = Database::connect(&url).await.unwrap();
    raw.execute_unprepared("CREATE VIEW records_finished AS SELECT 1")
        .await
        .unwrap();
    assert_eq!(
        SeaOrmStore::connect(&url).await.map(|_| ()),
        Err(Error::Unavailable)
    );
    assert_eq!(
        integer_query(&raw, "SELECT count(*) AS value FROM information_schema.tables WHERE table_schema = current_schema() AND table_type = 'BASE TABLE'").await,
        0
    );
    raw.execute_unprepared("DROP VIEW records_finished")
        .await
        .unwrap();
    SeaOrmStore::connect(&url)
        .await
        .unwrap()
        .close()
        .await
        .unwrap();
    assert_eq!(
        integer_query(&raw, "SELECT version AS value FROM schema_version").await,
        7
    );
}
