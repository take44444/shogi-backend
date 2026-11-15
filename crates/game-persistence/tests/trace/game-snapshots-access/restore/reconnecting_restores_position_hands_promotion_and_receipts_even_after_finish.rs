#[tokio::test]
async fn reconnecting_restores_position_hands_promotion_and_receipts_even_after_finish() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let (store, service) = setup(&url).await;
    let first = movement("m0", 0, "black", "7g7f");
    let receipt = service.make_move(&id(), first.clone()).await.unwrap();
    for (i, (name, usi)) in [
        ("white", "3c3d"),
        ("black", "8h2b+"),
        ("white", "3a2b"),
        ("black", "B*4e"),
    ]
    .into_iter()
    .enumerate()
    {
        service
            .make_move(
                &id(),
                movement(&format!("m{}", i + 1), i as u64 + 1, name, usi),
            )
            .await
            .unwrap();
    }
    let ongoing = service.snapshot(&id()).await.unwrap();
    drop(service);
    store.close().await.unwrap();
    let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
    let service = pg_test_support::GameFixture::new(store.clone(), &url);
    assert_eq!(service.snapshot(&id()).await.unwrap(), ongoing);
    assert_eq!(service.make_move(&id(), first.clone()).await, Ok(receipt));
    let finish = service.resign(&id(), resign()).await.unwrap();
    assert_eq!(finish.result.unwrap().winner, Player::White);
    let finished = service.snapshot(&id()).await.unwrap();
    drop(service);
    store.close().await.unwrap();
    let service = pg_test_support::GameFixture::new(
        Arc::new(SeaOrmStore::connect(&url).await.unwrap()),
        &url,
    );
    assert_eq!(service.snapshot(&id()).await.unwrap(), finished);
    assert_eq!(service.make_move(&id(), first).await, Ok(receipt));
    assert_eq!(service.resign(&id(), resign()).await, Ok(finish));
    assert_eq!(
        service
            .make_move(&id(), movement("m0", 0, "white", "7g7f"))
            .await,
        Err(Error::RequestConflict)
    );
}
