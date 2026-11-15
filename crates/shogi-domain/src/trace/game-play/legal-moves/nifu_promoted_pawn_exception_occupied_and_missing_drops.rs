crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Nifu, promoted-pawn exception, occupied-square and unavailable-hand drop cases are asserted.",
        reason: "Nifu, promoted-pawn exception, occupied-square and unavailable-hand drop cases are asserted.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn nifu_promoted_pawn_exception_occupied_and_missing_drops() {
    for (sfen, player, nifu, allowed, occupied) in [
        (
            "k8/9/9/9/4P4/9/9/9/8K b P 1",
            Player::Black,
            "P*5d",
            "P*4d",
            "P*5e",
        ),
        (
            "k8/9/9/9/4p4/9/9/9/8K w p 1",
            Player::White,
            "P*5f",
            "P*4f",
            "P*5e",
        ),
    ] {
        let mut game = Game::from_sfen(sfen);
        reject(&mut game, player, nifu, CommandError::IllegalMove);
        reject(&mut game, player, occupied, CommandError::IllegalMove);
        reject(&mut game, player, "R*4e", CommandError::IllegalMove);
        play(&mut game, allowed);
    }
    for (sfen, movement) in [
        ("k8/9/9/9/4+P4/9/9/9/8K b P 1", "P*5d"),
        ("k8/9/9/9/4+p4/9/9/9/8K w p 1", "P*5f"),
    ] {
        play(&mut Game::from_sfen(sfen), movement);
    }
}
}
