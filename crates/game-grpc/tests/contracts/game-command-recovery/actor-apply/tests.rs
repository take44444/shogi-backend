use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/grpc-command", relationship: crate::trace_support::Relationship::Unreviewed, scope: "製品の公開 API を呼ぶ結合テスト。個々の assertion と契約の対応は未レビュー。", reason: "宣言した実提供元の境界を利用する。契約の各条件を直接検証するかは未レビュー。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-application"], fake: &[] }] };
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn independent_tcp_clients_serialize_competing_commands_and_retry_identically() {
    let server = TestServer::start().await;
    let mut first = server.client().await;
    let mut second = server.client().await;
    create(&mut first, "concurrent").await;
    let a = command("concurrent", "a", 0, proto::Player::Black, "7g7f");
    let b = command("concurrent", "b", 0, proto::Player::Black, "2g2f");
    let (left, right) = tokio::join!(first.make_move(a), second.make_move(b));
    let error = match (left, right) {
        (Ok(receipt), Err(error)) | (Err(error), Ok(receipt)) => {
            assert_eq!(receipt.into_inner().seq, 1);
            error
        }
        result => panic!("expected exactly one acceptance, got {result:?}"),
    };
    assert_eq!(error.code(), Code::Aborted);
    assert_eq!(error.metadata().get("actual-seq").unwrap(), "1");
    let retry = command("concurrent", "white", 1, proto::Player::White, "3c3d");
    let (left, right) = tokio::join!(first.make_move(retry.clone()), second.make_move(retry));
    assert_eq!(left.unwrap().into_inner(), right.unwrap().into_inner());
    assert_eq!(snapshot(&mut first, "concurrent").await.seq, 2);
    let other = second
        .seed_game(SeedGame {
            game_id: "other".into(),
            black_user_id: "other-black".into(),
            white_user_id: "other-white".into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(other.seq, 0);
    assert_eq!(other.board.len(), 40);
    drop(first);
    drop(second);
    server.shutdown().await;
}
}
