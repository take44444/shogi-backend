crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Sliders stop at friendly pieces and cannot move beyond captured enemies.",
        reason: "Sliders stop at friendly pieces and cannot move beyond captured enemies.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn sliders_stop_at_friendly_and_enemy_blockers() {
    for (sfen, from, legal, illegal) in [
        ("k8/9/9/4P4/4R4/9/9/9/8K b - 1", "5e", "5e5f", "5e5b"),
        ("k8/9/9/4p4/4R4/9/9/9/8K b - 1", "5e", "5e5d", "5e5c"),
        ("k8/9/2P6/9/4B4/9/9/9/8K b - 1", "5e", "5e6d", "5e8b"),
        ("k8/9/9/4P4/4L4/9/9/9/8K b - 1", "5e", "5e5e", "5e5c"),
    ] {
        let mut game = Game::from_sfen(sfen);
        assert!(game.position().piece_at(sq(from)).is_some());
        if legal != "5e5e" {
            assert!(game.legal_moves().contains(&mv(legal)));
        }
        reject(&mut game, Player::Black, illegal, CommandError::IllegalMove);
    }
}
}
