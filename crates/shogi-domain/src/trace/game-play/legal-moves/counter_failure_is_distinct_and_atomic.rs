crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. RulesFailure is distinct and preserves state on a rules-engine counter failure.",
        reason: "RulesFailure is distinct and preserves state on a rules-engine counter failure.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn counter_failure_is_distinct_and_atomic() {
    let mut game = Game::from_sfen("4k4/9/9/9/9/9/9/9/4K4 b - 65535");
    assert!(game.legal_moves().contains(&mv("5i4i")));
    reject(&mut game, Player::Black, "5i4i", CommandError::RulesFailure);
}
}
