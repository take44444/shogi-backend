#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn competing_commands_at_one_sequence_accept_exactly_one() {
    let (_database, service) = setup().await;
    let game = id("race");
    service.seed(game.clone(), participants()).await.unwrap();
    let mut tasks = JoinSet::new();
    let barrier = Arc::new(Barrier::new(32));
    for index in 0..32 {
        let service = service.clone();
        let game = game.clone();
        let barrier = barrier.clone();
        tasks.spawn(async move {
            barrier.wait().await;
            service
                .make_move(
                    &game,
                    movement(
                        &format!("request-{index}"),
                        0,
                        Player::Black,
                        if index % 2 == 0 { "7g7f" } else { "2g2f" },
                    ),
                )
                .await
        });
    }
    let mut accepted = 0;
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(receipt) => {
                accepted += 1;
                assert_eq!(receipt.seq, 1);
            }
            Err(error) => assert_eq!(
                error,
                Error::SequenceMismatch {
                    expected: 0,
                    actual: 1
                }
            ),
        }
    }
    assert_eq!(accepted, 1);
    assert_eq!(service.snapshot(&game).await.unwrap().seq, 1);
}
