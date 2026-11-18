use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/grpc-command", relationship: crate::trace_support::Relationship::Unreviewed, scope: "製品の公開 API を呼ぶ結合テスト。個々の assertion と契約の対応は未レビュー。", reason: "宣言した実提供元の境界を利用する。契約の各条件を直接検証するかは未レビュー。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-application"], fake: &[] }] };
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn competing_commands_at_one_sequence_accept_exactly_one() {
    let (_database, service) = setup().await;
    let game = id("race");
    service.seed(game.clone(), participants()).await.unwrap();
    let mut tasks = JoinSet::new();
    let barrier = Arc::new(Barrier::new(32));
    for index in 0..32 {
        let service = service.clone();
        let game = game.clone();
        let barrier = barrier.clone();
        tasks.spawn(async move {
            barrier.wait().await;
            service
                .make_move(
                    &game,
                    movement(
                        &format!("request-{index}"),
                        0,
                        Player::Black,
                        if index % 2 == 0 { "7g7f" } else { "2g2f" },
                    ),
                )
                .await
        });
    }
    let mut accepted = 0;
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(receipt) => {
                accepted += 1;
                assert_eq!(receipt.seq, 1);
            }
            Err(error) => assert_eq!(
                error,
                Error::SequenceMismatch {
                    expected: 0,
                    actual: 1
                }
            ),
        }
    }
    assert_eq!(accepted, 1);
    assert_eq!(service.snapshot(&game).await.unwrap().seq, 1);
}
}
