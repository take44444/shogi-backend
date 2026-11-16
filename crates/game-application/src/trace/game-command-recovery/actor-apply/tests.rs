use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "記録用のメモリー内リポジトリを使ったアクター。コミット前のステージ済み値を検証し、PostgreSQL の原子性は検証しない。",
        reason: "テスト用リポジトリで、コミット前の着手と投了のステージ済みイベントが正確であり、再試行と拒否があっても書き込みは二回だけであることを確認する。",
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

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "合成した永続記録とテスト用リポジトリを使ったアクターで、エラーの優先順位を直接検証する。",
        reason: "合成したアクターの事例で、リクエスト競合、シーケンス不一致、終局後の拒否の順に優先されることを検証する。",
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
