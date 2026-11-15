crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. No legal moves without check leaves game ongoing.",
        reason: "A side with no legal moves but no check does not automatically lose.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn no_legal_moves_without_check_is_not_checkmate() {
    let game = Game::from_sfen("8k/6R2/7R1/9/9/9/9/9/K8 w - 1");
    assert!(!game.in_check());
    assert!(game.legal_moves().is_empty());
    assert_eq!(game.result(), None);
    let mut game = Game::from_sfen("8k/6R2/9/7R1/9/9/9/9/K8 b - 1");
    assert_eq!(
        play(&mut game, "2d2c"),
        MoveOutcome {
            in_check: false,
            result: None
        }
    );
    assert!(game.legal_moves().is_empty());
    assert_eq!(
        game.resign(Player::White).unwrap().reason,
        FinishReason::Resignation
    );
}
}
