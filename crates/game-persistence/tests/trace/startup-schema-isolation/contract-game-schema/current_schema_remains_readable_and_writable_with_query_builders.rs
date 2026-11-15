#[tokio::test]
async fn current_schema_remains_readable_and_writable_with_query_builders() {
    let database = pg_test_support::TestDatabase::new().await;
    let raw = Database::connect(database.url()).await.unwrap();
    raw.execute_unprepared(include_str!("../../../fixtures/schema-v7.sql"))
        .await
        .unwrap();
    let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
    let games = pg_test_support::GameFixture::new(store.clone(), database.url());
    let id = WriteId::new("existing-schema").unwrap();
    games
        .seed(
            id.clone(),
            Participants::new(UserId::new("black").unwrap(), UserId::new("white").unwrap())
                .unwrap(),
        )
        .await
        .unwrap();
    games
        .resign(
            &id,
            Resign {
                request_id: RequestId::new("end").unwrap(),
                expected_seq: 0,
                user_id: UserId::new("white").unwrap(),
            },
        )
        .await
        .unwrap();
    let reader = PostgresGameRecords::connect(&database.reader_url().await)
        .await
        .unwrap();
    let detail = reader
        .detail(&GameId::new(id.as_str()).unwrap())
        .await
        .unwrap();
    assert_eq!(detail.metadata.final_seq().get(), 1);
    assert_eq!(detail.history[0].action, PublicAction::Resign);
    reader.close().await;
    store.close().await.unwrap();
}
