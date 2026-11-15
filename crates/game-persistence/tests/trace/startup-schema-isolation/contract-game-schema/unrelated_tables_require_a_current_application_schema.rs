#[tokio::test]
async fn unrelated_tables_require_a_current_application_schema() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let raw = Database::connect(&url).await.unwrap();
    raw.execute_unprepared(
        "CREATE TABLE application_extra (value TEXT); INSERT INTO application_extra VALUES ('preserved')",
    )
    .await
    .unwrap();
    assert_eq!(
        SeaOrmStore::connect(&url).await.map(|_| ()),
        Err(Error::Corrupt)
    );
    assert_eq!(
        integer_query(
            &raw,
            "SELECT count(*) AS value FROM information_schema.tables WHERE table_schema = current_schema() AND table_type = 'BASE TABLE'"
        )
        .await,
        1
    );
    raw.execute_unprepared("DROP TABLE application_extra")
        .await
        .unwrap();
    SeaOrmStore::connect(&url)
        .await
        .unwrap()
        .close()
        .await
        .unwrap();
    raw.execute_unprepared(
        "CREATE TABLE application_extra (value TEXT); INSERT INTO application_extra VALUES ('preserved')",
    )
    .await
    .unwrap();
    SeaOrmStore::connect(&url)
        .await
        .unwrap()
        .close()
        .await
        .unwrap();
    assert_eq!(
        integer_query(
            &raw,
            "SELECT count(*) AS value FROM application_extra WHERE value = 'preserved'"
        )
        .await,
        1
    );
}
