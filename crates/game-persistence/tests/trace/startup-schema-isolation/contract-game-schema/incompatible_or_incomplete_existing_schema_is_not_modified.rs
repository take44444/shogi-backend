#[tokio::test]
async fn incompatible_or_incomplete_existing_schema_is_not_modified() {
    for sql in [
        "UPDATE schema_version SET version = 0",
        "UPDATE schema_version SET version = 4",
        "UPDATE schema_version SET id = 1",
        "INSERT INTO schema_version VALUES (1, 5)",
        "ALTER TABLE schema_version RENAME COLUMN version TO missing",
        "DROP TABLE schema_version; CREATE TABLE schema_version (id INTEGER, version TEXT); INSERT INTO schema_version VALUES (0, 'invalid')",
        "DROP TABLE schema_version; CREATE TABLE schema_version (id TEXT, version INTEGER); INSERT INTO schema_version VALUES ('invalid', 5)",
        "DROP TABLE outbox_events",
    ] {
        let database = TestDatabase::new().await;
        let url = database.url().to_string();
        let (_, service) = setup(&url).await;
        service
            .make_move(&id(), movement("m", 0, "black", "7g7f"))
            .await
            .unwrap();
        let raw = Database::connect(&url).await.unwrap();
        raw.execute_unprepared(sql).await.unwrap();
        assert_eq!(
            SeaOrmStore::connect(&url).await.map(|_| ()),
            Err(Error::Corrupt),
            "{sql}"
        );
        assert_eq!(
            integer_query(&raw, "SELECT count(*) AS value FROM games").await,
            1
        );
        assert_eq!(
            integer_query(&raw, "SELECT count(*) AS value FROM journal").await,
            1
        );
    }
}
