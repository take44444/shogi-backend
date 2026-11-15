#[tokio::test]
async fn an_existing_empty_schema_ledger_is_rejected_without_modification() {
    for existing_tables in [false, true] {
        let database = TestDatabase::new().await;
        let url = database.url().to_string();
        if existing_tables {
            SeaOrmStore::connect(&url)
                .await
                .unwrap()
                .close()
                .await
                .unwrap();
        }
        let raw = Database::connect(&url).await.unwrap();
        if existing_tables {
            raw.execute_unprepared("DELETE FROM schema_version")
                .await
                .unwrap();
        } else {
            raw.execute_unprepared("CREATE TABLE schema_version (id INTEGER PRIMARY KEY NOT NULL, version INTEGER NOT NULL)").await.unwrap();
        }
        let actual = SeaOrmStore::connect(&url).await.map(|_| ());
        assert_eq!(
            actual,
            Err(Error::Corrupt),
            "existing_tables={existing_tables}"
        );
        let row = raw
            .query_one_raw(sea_orm::Statement::from_string(
                sea_orm::DbBackend::Postgres,
                "SELECT COUNT(*) AS count FROM schema_version",
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.try_get::<i64>("", "count").unwrap(), 0);
    }
}
