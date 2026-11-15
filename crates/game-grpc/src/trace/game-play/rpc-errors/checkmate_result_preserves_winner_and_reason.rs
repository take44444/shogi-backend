crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct terminal-result enum conversion; does not play a network checkmate sequence.",
        reason: "Checkmate result conversion preserves winner and reason.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
    fn checkmate_result_preserves_winner_and_reason() {
        let result = proto::GameResult::from(GameResult {
            winner: Player::White,
            reason: FinishReason::Checkmate,
        });
        assert_eq!(result.winner, proto::Player::White as i32);
        assert_eq!(result.reason, proto::FinishReason::Checkmate as i32);
    }
}
