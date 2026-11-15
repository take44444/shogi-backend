crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport.",
        reason: "Clones retain independent position and result through move/resign mutations.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn cloned_games_own_their_position_and_terminal_result() {
    let original = Game::new();
    let initial = original.position();
    let mut staged = original.clone();
    staged.play(Player::Black, "7g7f".parse().unwrap()).unwrap();
    assert_eq!(original.position(), initial);
    assert_ne!(staged.position(), initial);
    staged.resign(Player::Black).unwrap();
    assert_eq!(original.result(), None);
    let finished = staged.clone();
    assert_eq!(finished.result(), staged.result());
    assert_eq!(finished.position(), staged.position());
    assert!(finished.legal_moves().is_empty());
}
}
