crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Promotion is allowed when a move starts in the zone and exits it.",
        reason: "Promotion is allowed when a move starts in the zone and exits it.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn promotion_can_start_in_zone_and_end_outside_it() {
    for (sfen, movement, kind) in [
        (
            "k8/9/4S4/9/9/9/9/9/8K b - 1",
            "5c4d+",
            PieceKind::PromotedSilver,
        ),
        (
            "k8/9/9/9/9/9/4s4/9/8K w - 1",
            "5g4f+",
            PieceKind::PromotedSilver,
        ),
        ("k8/9/4B4/9/9/9/9/9/8K b - 1", "5c4d+", PieceKind::Horse),
        ("k8/9/9/9/9/9/4r4/9/8K w - 1", "5g5f+", PieceKind::Dragon),
    ] {
        let mut game = Game::from_sfen(sfen);
        let plain = &movement[..4];
        assert!(game.legal_moves().contains(&mv(plain)));
        play(&mut game, movement);
        assert_eq!(
            game.position().piece_at(sq(&movement[2..4])).unwrap().kind,
            kind
        );
    }
}
}
