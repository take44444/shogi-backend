crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Mate sets expected winner/reason and finished-state rejection precedes turn/legality checks.",
        reason: "Checkmate returns the terminal winner/reason and later errors prioritize GameFinished.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn checkmate_finishes_and_finished_precedes_turn_or_legality() {
    let mut game = Game::from_sfen("3pkp3/3p1p3/4G4/9/9/9/9/9/K3R4 b - 1");
    let result = GameResult {
        winner: Player::Black,
        reason: FinishReason::Checkmate,
    };
    assert_eq!(
        play(&mut game, "5c5b"),
        MoveOutcome {
            in_check: true,
            result: Some(result)
        }
    );
    assert_eq!(game.turn(), Player::White);
    assert!(game.in_check());
    assert!(game.legal_moves().is_empty());
    for player in [Player::Black, Player::White] {
        reject(&mut game, player, "5a5b", CommandError::GameFinished);
        let before = snapshot(&game);
        assert_eq!(game.resign(player), Err(CommandError::GameFinished));
        assert_eq!(snapshot(&game), before);
    }
}
}
