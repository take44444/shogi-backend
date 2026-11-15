use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Pure receipt-to-event construction, no PostgreSQL or transport.",
        reason: "Direct assertions compare canonical move, mate and resignation facts and reject inconsistent action/result combinations.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "game-command-recovery", contract: "canonical-events", scope: "Direct StagedGameChange construction with synthetic receipts; no played checkmate or database.", relationship: crate::trace_support::Relationship::Asserts, reason: "Exact normal move, same-sequence move-then-mate-finish and resignation batches are compared; inconsistent action/result cases reject." }],
    };
#[test]
    fn canonical_move_mate_and_resignation_events() {
        let movement = "7g7f".parse().unwrap();
        let event = |event| CommittedEvent {
            game_id: game(),
            request_id: RequestId::new("request").unwrap(),
            user_id: UserId::new("black").unwrap(),
            seq: 1,
            event,
        };
        let normal = StagedGameChange::new(game(), receipt(Action::Move(movement), None)).unwrap();
        assert_eq!(
            normal.events(),
            &[event(DomainEvent::MoveCommitted {
                movement,
                in_check: false,
                result: None,
            })]
        );
        let mate = GameResult {
            winner: Player::Black,
            reason: FinishReason::Checkmate,
        };
        let terminal =
            StagedGameChange::new(game(), receipt(Action::Move(movement), Some(mate))).unwrap();
        assert_eq!(
            terminal.events(),
            &[
                event(DomainEvent::MoveCommitted {
                    movement,
                    in_check: true,
                    result: Some(mate),
                }),
                event(DomainEvent::GameFinished { result: mate }),
            ]
        );
        let result = GameResult {
            reason: FinishReason::Resignation,
            ..mate
        };
        let end = StagedGameChange::new(game(), receipt(Action::Resign, Some(result))).unwrap();
        assert_eq!(end.events(), &[event(DomainEvent::GameFinished { result })]);
        for bad in [
            receipt(Action::Resign, None),
            receipt(Action::Resign, Some(mate)),
            receipt(Action::Move(movement), Some(result)),
        ] {
            assert_eq!(
                StagedGameChange::new(game(), bad),
                Err(ApplicationError::Corrupt)
            );
        }
    }
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Pure staged sequence boundary and continuity cases.",
        reason: "Direct sequence cases check storage limits before continuity and reject nonconsecutive accepted sequences.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "game-command-recovery", contract: "canonical-events", scope: "Direct synthetic receipt sequence boundaries.", relationship: crate::trace_support::Relationship::Asserts, reason: "Zero/noncontiguous/max/out-of-range acceptance and expected sequence boundaries are checked." }],
    };
#[test]
    fn staging_checks_sequence_bounds_before_continuity() {
        let limit = i64::MAX as u64;
        for (expected, accepted, error) in [
            (limit, limit, ApplicationError::SequenceExhausted),
            (limit, limit + 1, ApplicationError::SequenceExhausted),
            (u64::MAX, 0, ApplicationError::SequenceExhausted),
            (0, limit + 1, ApplicationError::SequenceExhausted),
            (0, u64::MAX, ApplicationError::SequenceExhausted),
            (0, 0, ApplicationError::Corrupt),
            (0, 2, ApplicationError::Corrupt),
            (1, 1, ApplicationError::Corrupt),
        ] {
            let mut receipt = receipt(Action::Move("7g7f".parse().unwrap()), None);
            receipt.request.expected_seq = expected;
            receipt.acceptance.seq = accepted;
            assert_eq!(StagedGameChange::new(game(), receipt), Err(error));
        }
        let mut receipt = receipt(Action::Move("7g7f".parse().unwrap()), None);
        receipt.request.expected_seq = limit - 1;
        receipt.acceptance.seq = limit;
        let change = StagedGameChange::new(game(), receipt.clone()).unwrap();
        assert_eq!(change.game_id(), &game());
        assert_eq!(change.receipt(), &receipt);
        assert_eq!(change.events()[0].seq, limit);
    }
}
