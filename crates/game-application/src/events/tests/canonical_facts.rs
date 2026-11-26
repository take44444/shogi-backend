use super::*;

#[test]
fn canonical_move_mate_and_resignation_events() {
    let movement = "7g7f".parse().unwrap();
    let event = |event| CommittedEvent {
        clock: crate::ClockState::start(
            crate::TimeControl::for_pool(crate::rating::Pool::TenMinute30),
            crate::Timestamp::new(0).unwrap(),
        )
        .unwrap(),
        evaluated_at: crate::Timestamp::new(0).unwrap(),
        game_id: game(),
        origin: crate::EventOrigin::User {
            request_id: RequestId::new("request").unwrap(),
            user_id: UserId::new("black").unwrap(),
        },
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
    assert_eq!(change.entry(), &crate::JournalEntry::UserCommand(receipt));
    assert_eq!(change.events()[0].seq, limit);
}
