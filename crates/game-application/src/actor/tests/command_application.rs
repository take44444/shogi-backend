use super::*;

#[tokio::test]
async fn actor_supplies_events_before_locked_commit_and_skips_retries_and_rejections() {
    let store = store();
    let actor = GameService::new(store.clone());
    let first = receipt();
    let accepted = actor.mutate(&id(), first.clone()).await.unwrap();
    assert_eq!(actor.mutate(&id(), first.clone()).await, Ok(accepted));
    assert_eq!(
        actor
            .mutate(
                &id(),
                MutationRequest {
                    request_id: RequestId::new("rejected").unwrap(),
                    expected_seq: 1,
                    ..first.clone()
                }
            )
            .await,
        Err(ApplicationError::Domain(
            shogi_domain::CommandError::WrongTurn
        ))
    );
    let resignation = MutationRequest {
        request_id: RequestId::new("end").unwrap(),
        user_id: UserId::new("white").unwrap(),
        expected_seq: 1,
        action: Action::Resign,
    };
    let finished = actor.mutate(&id(), resignation.clone()).await.unwrap();
    assert_eq!(actor.mutate(&id(), resignation.clone()).await, Ok(finished));

    let supplied = store.1.lock().unwrap();
    assert_eq!(supplied.len(), 2);
    assert_eq!(supplied[0].game_id(), &id());
    assert_eq!(supplied[0].entry().acceptance(), accepted);
    assert_eq!(
        supplied[0].events(),
        &[CommittedEvent {
            clock: accepted.clock,
            evaluated_at: crate::Timestamp::new(0).unwrap(),
            game_id: id(),
            origin: crate::EventOrigin::User {
                request_id: first.request_id,
                user_id: first.user_id
            },
            seq: accepted.seq,
            event: DomainEvent::MoveCommitted {
                movement: "7g7f".parse().unwrap(),
                in_check: accepted.in_check,
                result: accepted.result,
            },
        }]
    );
    assert_eq!(supplied[1].entry().acceptance(), finished);
    assert_eq!(
        supplied[1].events(),
        &[CommittedEvent {
            clock: finished.clock,
            evaluated_at: crate::Timestamp::new(0).unwrap(),
            game_id: id(),
            origin: crate::EventOrigin::User {
                request_id: resignation.request_id,
                user_id: resignation.user_id
            },
            seq: finished.seq,
            event: DomainEvent::GameFinished {
                result: finished.result.unwrap(),
            },
        }]
    );
}

#[tokio::test]
async fn mutation_errors_prioritize_request_conflict_then_sequence_mismatch_then_game_finished() {
    let service = GameService::new(store());
    let mut command = MakeMove {
        request_id: RequestId::new("oversized").unwrap(),
        user_id: UserId::new("black").unwrap(),
        expected_seq: u64::MAX,
        movement: "7g7f".parse().unwrap(),
    };
    assert_eq!(
        service.make_move(&id(), command.clone()).await,
        Err(ApplicationError::SequenceMismatch {
            expected: u64::MAX,
            actual: 0
        })
    );
    command.expected_seq = 0;
    assert_eq!(
        service.make_move(&id(), command.clone()).await.unwrap().seq,
        1
    );
    command.expected_seq = u64::MAX;
    assert_eq!(
        service.make_move(&id(), command.clone()).await,
        Err(ApplicationError::RequestConflict)
    );
    service
        .resign(
            &id(),
            crate::Resign {
                request_id: RequestId::new("finish").unwrap(),
                user_id: UserId::new("white").unwrap(),
                expected_seq: 1,
            },
        )
        .await
        .unwrap();
    command.request_id = RequestId::new("after-finish").unwrap();
    assert_eq!(
        service.make_move(&id(), command.clone()).await,
        Err(ApplicationError::SequenceMismatch {
            expected: u64::MAX,
            actual: 2
        })
    );
    command.expected_seq = 2;
    assert_eq!(
        service.make_move(&id(), command).await,
        Err(ApplicationError::Domain(
            shogi_domain::CommandError::GameFinished
        ))
    );
}

#[test]
fn late_requests_adjudicate_before_sequence_turn_and_legality_but_rejections_keep_deadline() {
    let repository = store();
    let record = repository.0.lock().unwrap().clone();
    let mut state = State::restore(record).unwrap();
    let clock = state.clock;
    let mut illegal = receipt();
    illegal.action = Action::Move("7g7e".parse().unwrap());
    assert!(matches!(
        state.stage(id(), Some(&illegal), Timestamp::new(629_999).unwrap()),
        Err(ApplicationError::Domain(
            shogi_domain::CommandError::IllegalMove
        ))
    ));
    assert_eq!(state.clock, clock);
    illegal.expected_seq = 99;
    let timeout = state
        .stage(id(), Some(&illegal), Timestamp::new(630_000).unwrap())
        .unwrap()
        .unwrap();
    assert!(matches!(
        timeout.entry(),
        JournalEntry::Timeout {
            loser: shogi_domain::Player::Black,
            ..
        }
    ));
    assert_eq!(
        timeout.entry().acceptance().result.unwrap().reason,
        shogi_domain::FinishReason::Timeout
    );
    assert!(matches!(
        timeout.events()[0].origin,
        crate::EventOrigin::System
    ));
}

#[test]
fn resignation_charges_side_to_move_even_when_opponent_resigns() {
    let repository = store();
    let mut state = State::restore(repository.0.lock().unwrap().clone()).unwrap();
    let resign = MutationRequest {
        action: Action::Resign,
        user_id: UserId::new("white").unwrap(),
        ..receipt()
    };
    let change = state
        .stage(id(), Some(&resign), Timestamp::new(600_125).unwrap())
        .unwrap()
        .unwrap();
    match change.entry().acceptance().clock {
        ClockState::Stopped {
            main_remaining_ms,
            byoyomi_remaining_ms,
            stopped_at,
        } => {
            assert_eq!(main_remaining_ms.map(|m| m.get()), [0, 600_000]);
            assert_eq!(byoyomi_remaining_ms.map(|m| m.get()), [29_875, 30_000]);
            assert_eq!(stopped_at.get(), 600_125);
        }
        _ => panic!("finished clock must stop"),
    }
}
