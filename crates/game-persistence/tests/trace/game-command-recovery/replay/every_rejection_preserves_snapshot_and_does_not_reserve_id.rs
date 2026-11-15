crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport.",
        reason: "Every tested rejection preserves full snapshot and leaves the request ID reusable.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn every_rejection_preserves_snapshot_and_does_not_reserve_id() {
    let (_database, service) = setup().await;
    let game = id("rejection");
    let before = service.seed(game.clone(), participants()).await.unwrap();
    for (command, expected) in [
        (
            movement("correct-me", 2, Player::Black, "7g7f"),
            Error::SequenceMismatch {
                expected: 2,
                actual: 0,
            },
        ),
        (
            movement("correct-me", 0, Player::White, "3c3d"),
            Error::Domain(CommandError::WrongTurn),
        ),
        (
            movement("correct-me", 0, Player::Black, "7g7e"),
            Error::Domain(CommandError::IllegalMove),
        ),
    ] {
        assert_eq!(service.make_move(&game, command).await, Err(expected));
        assert_eq!(service.snapshot(&game).await.unwrap(), before);
    }
    let receipt = service
        .resign(&game, resignation("correct-me", 0, Player::White))
        .await
        .unwrap();
    assert_eq!(receipt.seq, 1);
    assert_eq!(receipt.result.unwrap().winner, Player::Black);
    let finished = service.snapshot(&game).await.unwrap();
    assert_eq!(finished.position, before.position);
    assert_eq!(finished.turn, before.turn);
    assert!(finished.legal_moves.is_empty());
    assert_eq!(
        service
            .make_move(&game, movement("after", 1, Player::Black, "7g7f"))
            .await,
        Err(Error::Domain(CommandError::GameFinished))
    );
    assert_eq!(
        service
            .resign(&game, resignation("after", 1, Player::Black))
            .await,
        Err(Error::Domain(CommandError::GameFinished))
    );
    assert_eq!(service.snapshot(&game).await.unwrap(), finished);
}
}
