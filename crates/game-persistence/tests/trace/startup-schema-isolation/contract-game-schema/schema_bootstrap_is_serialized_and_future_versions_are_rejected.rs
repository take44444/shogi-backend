#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn schema_bootstrap_is_serialized_and_future_versions_are_rejected() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..6 {
        let url = url.to_owned();
        tasks.spawn(async move { SeaOrmStore::connect(&url).await });
    }
    while let Some(result) = tasks.join_next().await {
        result.unwrap().unwrap();
    }
    let raw = Database::connect(&url).await.unwrap();
    raw.execute_unprepared("UPDATE schema_version SET version = 99")
        .await
        .unwrap();
    assert!(matches!(
        SeaOrmStore::connect(&url).await,
        Err(Error::Corrupt)
    ));
}
