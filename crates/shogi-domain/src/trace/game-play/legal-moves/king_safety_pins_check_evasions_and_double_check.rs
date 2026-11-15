crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Pins, legal check evasions and double-check legal moves are asserted.",
        reason: "Pins, legal check evasions and double-check legal moves are asserted.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn king_safety_pins_check_evasions_and_double_check() {
    let mut pinned = Game::from_sfen("k3r4/9/9/9/9/9/9/4G4/4K4 b - 1");
    assert!(!pinned.in_check());
    reject(
        &mut pinned,
        Player::Black,
        "5h4h",
        CommandError::IllegalMove,
    );
    assert!(pinned.legal_moves().contains(&mv("5h5g")));
    let mut checked = Game::from_sfen("k3r4/9/9/9/9/9/9/9/4K4 b GS 1");
    assert!(checked.in_check());
    reject(
        &mut checked,
        Player::Black,
        "S*4h",
        CommandError::IllegalMove,
    );
    reject(
        &mut checked,
        Player::Black,
        "5i5h",
        CommandError::IllegalMove,
    );
    play(&mut checked, "G*5h");
    assert!(!checked.in_check());
    let mut double = Game::from_sfen("k3r4/9/9/9/b8/9/9/9/4K4 b G 1");
    assert!(double.in_check());
    reject(
        &mut double,
        Player::Black,
        "G*5h",
        CommandError::IllegalMove,
    );
    assert!(
        double
            .legal_moves()
            .iter()
            .all(|m| matches!(m,Move::Board {from,..} if *from==sq("5i")))
    );
}
}
