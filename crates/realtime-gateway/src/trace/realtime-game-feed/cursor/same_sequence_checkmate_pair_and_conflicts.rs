crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct cursor state transitions over synthetic committed notifications; no socket or played mate.",
        reason: "Move then matching mate finish deliver, exact duplicates ignore and conflicting same-sequence move resyncs.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
    fn same_sequence_checkmate_pair_and_conflicts() {
        let result = GameResult {
            winner: Player::Black,
            reason: FinishReason::Checkmate,
        };
        let move_event = event(
            1,
            DomainEvent::MoveCommitted {
                movement: "7g7f".parse().unwrap(),
                in_check: true,
                result: Some(result),
            },
        );
        let finish = event(1, DomainEvent::GameFinished { result });
        let mut state = ConnectionState::Bootstrapping;
        state.bootstrapped(0, false);
        assert!(matches!(state.accept(&move_event), Decision::Deliver));
        assert!(matches!(state.accept(&move_event), Decision::Ignore));
        assert!(matches!(state.accept(&finish), Decision::Deliver));
        assert!(matches!(state.accept(&finish), Decision::Ignore));
        let changed = event(
            1,
            DomainEvent::MoveCommitted {
                movement: "2g2f".parse().unwrap(),
                in_check: true,
                result: Some(result),
            },
        );
        assert!(matches!(state.accept(&changed), Decision::Resync));
    }
}
