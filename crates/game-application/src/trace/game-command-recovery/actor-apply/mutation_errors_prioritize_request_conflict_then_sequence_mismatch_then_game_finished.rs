crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Actor with synthetic durable record and fake repository; direct error precedence.",
        reason: "Synthetic actor cases assert request-conflict before sequence mismatch before finished-game rejection.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}
