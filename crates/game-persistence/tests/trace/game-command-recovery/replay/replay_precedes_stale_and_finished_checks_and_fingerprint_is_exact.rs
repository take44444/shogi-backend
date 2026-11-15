#[tokio::test]
async fn replay_precedes_stale_and_finished_checks_and_fingerprint_is_exact() {
    let (_database, service) = setup().await;
    let game = id("ledger");
    service.seed(game.clone(), participants()).await.unwrap();
    let first = movement("first", 0, Player::Black, "7g7f");
    let receipt = service.make_move(&game, first.clone()).await.unwrap();
    service
        .make_move(&game, movement("second", 1, Player::White, "3c3d"))
        .await
        .unwrap();
    let resign_command = resignation("resign", 2, Player::White);
    let finish_receipt = service.resign(&game, resign_command.clone()).await.unwrap();
    assert_eq!(
        finish_receipt.result.unwrap().reason,
        FinishReason::Resignation
    );
    let finished = service.snapshot(&game).await.unwrap();
    assert_eq!(service.make_move(&game, first).await, Ok(receipt));
    assert_eq!(
        service.resign(&game, resign_command).await,
        Ok(finish_receipt)
    );
    for changed in [
        movement("first", 1, Player::Black, "7g7f"),
        movement("first", 0, Player::White, "7g7f"),
        movement("first", 0, Player::Black, "2g2f"),
    ] {
        assert_eq!(
            service.make_move(&game, changed).await,
            Err(Error::RequestConflict)
        );
    }
    assert_eq!(
        service
            .resign(&game, resignation("first", 0, Player::Black))
            .await,
        Err(Error::RequestConflict)
    );
    assert_eq!(
        service
            .make_move(&game, movement("resign", 2, Player::White, "3c3d"))
            .await,
        Err(Error::RequestConflict)
    );
    assert_eq!(service.snapshot(&game).await.unwrap(), finished);
}
