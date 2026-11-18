use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/grpc-command", relationship: crate::trace_support::Relationship::Unreviewed, scope: "製品の公開 API を呼ぶ結合テスト。個々の assertion と契約の対応は未レビュー。", reason: "宣言した実提供元の境界を利用する。契約の各条件を直接検証するかは未レビュー。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-application"], fake: &[] }] };
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_creation_has_exactly_one_winner_and_shared_state() {
    let (_database, service) = setup().await;
    let barrier = Arc::new(Barrier::new(24));
    let mut tasks = JoinSet::new();
    for _ in 0..24 {
        let service = service.clone();
        let barrier = barrier.clone();
        tasks.spawn(async move {
            barrier.wait().await;
            service.seed(id("same"), participants()).await
        });
    }
    let mut created = 0;
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(snapshot) => {
                created += 1;
                assert_eq!(snapshot.seq, 0);
            }
            Err(error) => assert_eq!(error, Error::AlreadyExists),
        }
    }
    assert_eq!(created, 1);
    service
        .clone()
        .make_move(&id("same"), movement("m", 0, Player::Black, "7g7f"))
        .await
        .unwrap();
    assert_eq!(service.snapshot(&id("same")).await.unwrap().seq, 1);
}
}
