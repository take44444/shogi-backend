crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Pawn-drop mate is rejected while a checking pawn drop with an escape remains legal.",
        reason: "Pawn-drop mate is rejected while a checking pawn drop with an escape remains legal.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn pawn_drop_mate_is_rejected_but_checking_pawn_with_escape_is_allowed() {
    let mut game = Game::from_sfen("3pkp3/3p1p3/4G4/9/9/9/9/9/K3R4 b P 1");
    reject(&mut game, Player::Black, "P*5b", CommandError::IllegalMove);
    let mut escape = Game::from_sfen("3pk4/3p1p3/4G4/9/9/9/9/9/K3R4 b P 1");
    let outcome = play(&mut escape, "P*5b");
    assert!(outcome.in_check);
    assert_eq!(outcome.result, None);
    assert!(escape.legal_moves().contains(&mv("5a4a")));
    play(&mut escape, "5a4a");
}
}
