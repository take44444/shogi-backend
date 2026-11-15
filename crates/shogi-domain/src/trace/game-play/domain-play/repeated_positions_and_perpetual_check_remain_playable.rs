crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Repeated positions and perpetual check continue without automatic adjudication.",
        reason: "Repeated and perpetual-check positions continue.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn repeated_positions_and_perpetual_check_remain_playable() {
    let mut game = Game::from_sfen("4k4/9/9/9/9/9/9/9/4K4 b - 1");
    let initial = game.position();
    for _ in 0..5 {
        for movement in ["5i4i", "5a4a", "4i5i", "4a5a"] {
            play(&mut game, movement);
        }
        assert_eq!(game.position(), initial);
        assert_eq!(game.result(), None);
    }
    let mut game = Game::from_sfen("4k4/9/5R3/9/9/9/9/9/K8 b - 1");
    for _ in 0..5 {
        for movement in ["4c5c", "5a4a", "5c4c", "4a5a"] {
            play(&mut game, movement);
        }
        assert_eq!(game.result(), None);
    }
}
}
