use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/grpc-command", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。実際のストアとサービスをそれぞれ独立したインスタンスで実行する。", reason: "独立した実際のストアとサービスのインスタンスで、CAS の競合と同じ内容の同時再試行を直列化する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-application"], fake: &[] }, crate::trace_support::Evidence { contract: "game-application/game-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。実際のストアとサービスをそれぞれ独立したインスタンスで実行する。", reason: "独立したサービスとストアのインスタンスで、CAS と状態更新が正しく動く。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn independent_instances_cas_refresh_and_identical_concurrent_replay() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let (store, a) = setup(&url).await;
    let b = pg_test_support::GameFixture::new(
        Arc::new(SeaOrmStore::connect(&url).await.unwrap()),
        &url,
    );
    assert_eq!(b.snapshot(&id()).await.unwrap().seq, 0);
    let game = id();
    let (left, right) = tokio::join!(
        a.make_move(&game, movement("a", 0, "black", "7g7f")),
        b.make_move(&game, movement("b", 0, "black", "2g2f"))
    );
    assert!(matches!(
        (left, right),
        (Ok(_), Err(Error::SequenceMismatch { actual: 1, .. }))
            | (Err(Error::SequenceMismatch { actual: 1, .. }), Ok(_))
    ));
    assert_eq!(
        a.snapshot(&game).await.unwrap(),
        b.snapshot(&game).await.unwrap()
    );
    let command = movement("white", 1, "white", "3c3d");
    let (left, right) = tokio::join!(
        a.make_move(&game, command.clone()),
        b.make_move(&game, command)
    );
    assert_eq!(left.unwrap(), right.unwrap());
    assert_eq!(store.load_game(&game).await.unwrap().journal.len(), 2);
    store.close().await.unwrap();
    assert_eq!(a.snapshot(&game).await, Err(Error::Unavailable));
    assert_eq!(
        a.make_move(&game, movement("white", 1, "white", "3c3d"))
            .await,
        Err(Error::Unavailable)
    );
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/grpc-command", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。FaultStore が実際の PostgreSQL を包み、返す結果と読み込み失敗を制御する。", reason: "コミット後にエラーが返る場合は受理記録と照合する。コミット前の失敗では、永続状態の読み込みが回復するまでローカル状態の使用を止める。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-application"], fake: &[] }, crate::trace_support::Evidence { contract: "game-application/game-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。FaultStore が実際の PostgreSQL を包み、返す結果と読み込み失敗を制御する。", reason: "実際の PostgreSQL を包む障害注入アダプターで、コミット後のエラーからの受理記録の復旧、コミット前の失敗、読み込み不能時の状態使用停止、その後の復旧を検証する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
#[tokio::test]
async fn uncertain_outcomes_reconcile_durable_receipts_or_fence_until_recovery() {
    for mode in [1, 2, 3, 4] {
        let database = TestDatabase::new().await;
        let (store, _) = setup(database.url()).await;
        let fault = Arc::new(FaultStore {
            inner: store.clone(),
            mode: AtomicU8::new(mode),
            unreadable: AtomicBool::new(false),
        });
        let service = pg_test_support::GameFixture::new(fault.clone(), database.url());
        let command = movement("request", 0, "black", "7g7f");
        let response = service.make_move(&id(), command.clone()).await;
        match mode {
            2 => assert_eq!(response.unwrap().seq, 1),
            4 => assert_eq!(response, Err(Error::RequestConflict)),
            _ => assert_eq!(response, Err(Error::Unavailable)),
        }
        if mode == 3 {
            assert_eq!(service.snapshot(&id()).await, Err(Error::Unavailable));
            assert_eq!(
                service
                    .resign(
                        &id(),
                        Resign {
                            request_id: RequestId::new("next").unwrap(),
                            expected_seq: 1,
                            user_id: user("white")
                        }
                    )
                    .await,
                Err(Error::Unavailable)
            );
            fault.unreadable.store(false, Ordering::SeqCst);
        }
        let record = store.load_game(&id()).await.unwrap();
        assert_eq!(record.journal.len(), usize::from(mode != 1));
        assert_eq!(
            service.snapshot(&id()).await.unwrap().seq,
            u64::from(mode != 1)
        );
        if mode != 4 {
            assert_eq!(service.make_move(&id(), command).await.unwrap().seq, 1);
        }
        assert_eq!(store.load_game(&id()).await.unwrap().journal.len(), 1);
    }
}
}
