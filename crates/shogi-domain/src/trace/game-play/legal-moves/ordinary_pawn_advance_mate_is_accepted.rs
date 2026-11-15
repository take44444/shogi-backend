crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. An ordinary advancing pawn may legally checkmate.",
        reason: "An ordinary advancing pawn may legally checkmate.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn ordinary_pawn_advance_mate_is_accepted() {
    let mut game = Game::from_sfen("3pkp3/3p1p3/4P4/9/9/9/9/9/K3R4 b - 1");
    let outcome = play(&mut game, "5c5b");
    assert!(outcome.in_check);
    assert_eq!(
        outcome.result,
        Some(GameResult {
            winner: Player::Black,
            reason: FinishReason::Checkmate
        })
    );
}
}
