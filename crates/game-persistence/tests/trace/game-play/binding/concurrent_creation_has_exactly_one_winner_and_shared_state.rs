#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_creation_has_exactly_one_winner_and_shared_state() {
    let (_database, service) = setup().await;
    let barrier = Arc::new(Barrier::new(24));
    let mut tasks = JoinSet::new();
    for _ in 0..24 {
        let service = service.clone();
        let barrier = barrier.clone();
        tasks.spawn(async move {
            barrier.wait().await;
            service.seed(id("same"), participants()).await
        });
    }
    let mut created = 0;
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(snapshot) => {
                created += 1;
                assert_eq!(snapshot.seq, 0);
            }
            Err(error) => assert_eq!(error, Error::AlreadyExists),
        }
    }
    assert_eq!(created, 1);
    service
        .clone()
        .make_move(&id("same"), movement("m", 0, Player::Black, "7g7f"))
        .await
        .unwrap();
    assert_eq!(service.snapshot(&id("same")).await.unwrap().seq, 1);
}
