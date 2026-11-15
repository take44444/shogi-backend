#[tokio::test]
async fn a_missing_schema_ledger_is_not_recreated_for_existing_game_tables() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    SeaOrmStore::connect(&url)
        .await
        .unwrap()
        .close()
        .await
        .unwrap();
    let raw = Database::connect(&url).await.unwrap();
    raw.execute_unprepared("DROP TABLE schema_version")
        .await
        .unwrap();
    assert_eq!(
        SeaOrmStore::connect(&url).await.map(|_| ()),
        Err(Error::Corrupt)
    );
    let row = raw
        .query_one_raw(sea_orm::Statement::from_string(
            sea_orm::DbBackend::Postgres,
            "SELECT COUNT(*) AS count FROM information_schema.tables WHERE table_schema = current_schema() AND table_name = 'schema_version'",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "count").unwrap(), 0);
}
