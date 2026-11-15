#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_identical_retries_all_receive_original_acceptance() {
    let (_database, service) = setup().await;
    let game = id("retry");
    service.seed(game.clone(), participants()).await.unwrap();
    let command = movement("same-request", 0, Player::Black, "7g7f");
    let mut tasks = JoinSet::new();
    let barrier = Arc::new(Barrier::new(32));
    for _ in 0..32 {
        let (service, game, command, barrier) = (
            service.clone(),
            game.clone(),
            command.clone(),
            barrier.clone(),
        );
        tasks.spawn(async move {
            barrier.wait().await;
            service.make_move(&game, command).await
        });
    }
    while let Some(result) = tasks.join_next().await {
        let acceptance = result.unwrap().unwrap();
        assert_eq!(acceptance.seq, 1);
        assert_eq!(acceptance.result, None);
    }
    assert_eq!(service.snapshot(&game).await.unwrap().seq, 1);
}
