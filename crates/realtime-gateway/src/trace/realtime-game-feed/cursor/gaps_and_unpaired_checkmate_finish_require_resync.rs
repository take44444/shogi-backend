crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct cursor boundary/gap fixtures; no broker or socket.",
        reason: "Old boundary event ignores; unpaired mate finish and sequence gap resync.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
    fn gaps_and_unpaired_checkmate_finish_require_resync() {
        let mut state = ConnectionState::Bootstrapping;
        state.bootstrapped(3, false);
        let result = GameResult {
            winner: Player::Black,
            reason: FinishReason::Checkmate,
        };
        assert!(matches!(
            state.accept(&event(3, DomainEvent::GameFinished { result })),
            Decision::Ignore
        ));
        assert!(matches!(
            state.accept(&event(4, DomainEvent::GameFinished { result })),
            Decision::Resync
        ));
        assert!(matches!(
            state.accept(&event(
                5,
                DomainEvent::MoveCommitted {
                    movement: "7g7f".parse().unwrap(),
                    in_check: false,
                    result: None
                }
            )),
            Decision::Resync
        ));
    }
}
