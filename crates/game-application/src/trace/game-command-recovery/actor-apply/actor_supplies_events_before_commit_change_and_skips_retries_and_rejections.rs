crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Actor with an in-memory recording repository; asserts staged values before commit, not PostgreSQL atomicity.",
        reason: "Fake repository observes exact staged move/resignation events before commit and only two writes despite replay/rejection.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn actor_supplies_events_before_commit_change_and_skips_retries_and_rejections() {
    let store = store();
    let mut actor = Actor::new(id(), store.clone());
    let first = receipt();
    let accepted = actor.apply(first.clone()).await.unwrap();
    assert_eq!(actor.apply(first.clone()).await, Ok(accepted));
    assert_eq!(
        actor
            .apply(MutationRequest {
                request_id: RequestId::new("rejected").unwrap(),
                expected_seq: 1,
                ..first.clone()
            })
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
    let finished = actor.apply(resignation.clone()).await.unwrap();
    assert_eq!(actor.apply(resignation.clone()).await, Ok(finished));

    let supplied = store.1.lock().unwrap();
    assert_eq!(supplied.len(), 2);
    assert_eq!(supplied[0].game_id(), &id());
    assert_eq!(supplied[0].receipt().acceptance, accepted);
    assert_eq!(
        supplied[0].events(),
        &[CommittedEvent {
            game_id: id(),
            request_id: first.request_id,
            user_id: first.user_id,
            seq: accepted.seq,
            event: DomainEvent::MoveCommitted {
                movement: "7g7f".parse().unwrap(),
                in_check: accepted.in_check,
                result: accepted.result,
            },
        }]
    );
    assert_eq!(supplied[1].receipt().acceptance, finished);
    assert_eq!(
        supplied[1].events(),
        &[CommittedEvent {
            game_id: id(),
            request_id: resignation.request_id,
            user_id: resignation.user_id,
            seq: finished.seq,
            event: DomainEvent::GameFinished {
                result: finished.result.unwrap(),
            },
        }]
    );
}
}
