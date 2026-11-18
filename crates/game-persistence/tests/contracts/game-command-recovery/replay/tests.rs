use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/grpc-command", relationship: crate::trace_support::Relationship::Unreviewed, scope: "製品の公開 API を呼ぶ結合テスト。個々の assertion と契約の対応は未レビュー。", reason: "宣言した実提供元の境界を利用する。契約の各条件を直接検証するかは未レビュー。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-application"], fake: &[] }] };
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_identical_retries_all_receive_original_acceptance() {
    let (_database, service) = setup().await;
    let game = id("retry");
    service.seed(game.clone(), participants()).await.unwrap();
    let command = movement("same-request", 0, Player::Black, "7g7f");
    let mut tasks = JoinSet::new();
    let barrier = Arc::new(Barrier::new(32));
    for _ in 0..32 {
        let (service, game, command, barrier) = (
            service.clone(),
            game.clone(),
            command.clone(),
            barrier.clone(),
        );
        tasks.spawn(async move {
            barrier.wait().await;
            service.make_move(&game, command).await
        });
    }
    while let Some(result) = tasks.join_next().await {
        let acceptance = result.unwrap().unwrap();
        assert_eq!(acceptance.seq, 1);
        assert_eq!(acceptance.result, None);
    }
    assert_eq!(service.snapshot(&game).await.unwrap().seq, 1);
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/grpc-command", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。", reason: "検証対象のすべての拒否で、スナップショット全体を保持し、リクエスト ID を再利用できる。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-application"], fake: &[] }] };
#[tokio::test]
async fn every_rejection_preserves_snapshot_and_does_not_reserve_id() {
    let (_database, service) = setup().await;
    let game = id("rejection");
    let before = service.seed(game.clone(), participants()).await.unwrap();
    for (command, expected) in [
        (
            movement("correct-me", 2, Player::Black, "7g7f"),
            Error::SequenceMismatch {
                expected: 2,
                actual: 0,
            },
        ),
        (
            movement("correct-me", 0, Player::White, "3c3d"),
            Error::Domain(CommandError::WrongTurn),
        ),
        (
            movement("correct-me", 0, Player::Black, "7g7e"),
            Error::Domain(CommandError::IllegalMove),
        ),
    ] {
        assert_eq!(service.make_move(&game, command).await, Err(expected));
        assert_eq!(service.snapshot(&game).await.unwrap(), before);
    }
    let receipt = service
        .resign(&game, resignation("correct-me", 0, Player::White))
        .await
        .unwrap();
    assert_eq!(receipt.seq, 1);
    assert_eq!(receipt.result.unwrap().winner, Player::Black);
    let finished = service.snapshot(&game).await.unwrap();
    assert_eq!(finished.position, before.position);
    assert_eq!(finished.turn, before.turn);
    assert!(finished.legal_moves.is_empty());
    assert_eq!(
        service
            .make_move(&game, movement("after", 1, Player::Black, "7g7f"))
            .await,
        Err(Error::Domain(CommandError::GameFinished))
    );
    assert_eq!(
        service
            .resign(&game, resignation("after", 1, Player::Black))
            .await,
        Err(Error::Domain(CommandError::GameFinished))
    );
    assert_eq!(service.snapshot(&game).await.unwrap(), finished);
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/grpc-command", relationship: crate::trace_support::Relationship::Unreviewed, scope: "製品の公開 API を呼ぶ結合テスト。個々の assertion と契約の対応は未レビュー。", reason: "宣言した実提供元の境界を利用する。契約の各条件を直接検証するかは未レビュー。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-application"], fake: &[] }] };
#[tokio::test]
async fn replay_precedes_stale_and_finished_checks_and_fingerprint_is_exact() {
    let (_database, service) = setup().await;
    let game = id("ledger");
    service.seed(game.clone(), participants()).await.unwrap();
    let first = movement("first", 0, Player::Black, "7g7f");
    let receipt = service.make_move(&game, first.clone()).await.unwrap();
    service
        .make_move(&game, movement("second", 1, Player::White, "3c3d"))
        .await
        .unwrap();
    let resign_command = resignation("resign", 2, Player::White);
    let finish_receipt = service.resign(&game, resign_command.clone()).await.unwrap();
    assert_eq!(
        finish_receipt.result.unwrap().reason,
        FinishReason::Resignation
    );
    let finished = service.snapshot(&game).await.unwrap();
    assert_eq!(service.make_move(&game, first).await, Ok(receipt));
    assert_eq!(
        service.resign(&game, resign_command).await,
        Ok(finish_receipt)
    );
    for changed in [
        movement("first", 1, Player::Black, "7g7f"),
        movement("first", 0, Player::White, "7g7f"),
        movement("first", 0, Player::Black, "2g2f"),
    ] {
        assert_eq!(
            service.make_move(&game, changed).await,
            Err(Error::RequestConflict)
        );
    }
    assert_eq!(
        service
            .resign(&game, resignation("first", 0, Player::Black))
            .await,
        Err(Error::RequestConflict)
    );
    assert_eq!(
        service
            .make_move(&game, movement("resign", 2, Player::White, "3c3d"))
            .await,
        Err(Error::RequestConflict)
    );
    assert_eq!(service.snapshot(&game).await.unwrap(), finished);
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/grpc-command", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。", reason: "別の対局やサービスで同じリクエスト識別子を使っても、それぞれ独立している。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-application"], fake: &[] }] };
#[tokio::test]
async fn games_and_independent_services_have_separate_id_namespaces() {
    let (_database, service) = setup().await;
    let (_other_database, other) = setup().await;
    let (a, b) = (id("a"), id("b"));
    let initial_a = service.seed(a.clone(), participants()).await.unwrap();
    let initial_b = service
        .seed(
            b.clone(),
            Participants::new(
                UserId::new("other-black").unwrap(),
                UserId::new("other-white").unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    other.seed(a.clone(), participants()).await.unwrap();
    service
        .make_move(&a, movement("shared", 0, Player::Black, "7g7f"))
        .await
        .unwrap();
    assert_eq!(service.snapshot(&b).await.unwrap(), initial_b);
    assert_eq!(other.snapshot(&a).await.unwrap(), initial_a);
    service
        .resign(
            &b,
            Resign {
                request_id: RequestId::new("shared").unwrap(),
                expected_seq: 0,
                user_id: UserId::new("other-white").unwrap(),
            },
        )
        .await
        .unwrap();
    assert!(service.snapshot(&a).await.unwrap().result.is_none());
    assert_eq!(service.snapshot(&id("missing")).await, Err(Error::NotFound));
    assert_eq!(
        service
            .make_move(&id("missing"), movement("m", 0, Player::Black, "7g7f"))
            .await,
        Err(Error::NotFound)
    );
    assert_eq!(
        service
            .resign(&id("missing"), resignation("r", 0, Player::White))
            .await,
        Err(Error::NotFound)
    );
}
}
