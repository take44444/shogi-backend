use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "生成されたgRPCクライアントをTCP経由で使い、GameRpc、アプリケーション、実際のPostgreSQLを通る。テスト専用の対局作成処理は事前準備として使う。",
        reason: "参加者以外の着手・投了は、既知のリクエストIDを再利用してもPermissionDeniedとなる。事前に作成した異なる参加者の割り当ても公開結果に保持される。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "game-play", contract: "rpc-resign", scope: "生成されたgRPCクライアントをTCP経由で使い、GameRpc、アプリケーション、実際のPostgreSQLを通る。テスト専用の対局作成処理は事前準備として使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "参加者以外の投了はPermissionDeniedとなる。" }],
    };
#[tokio::test]
async fn id_bindings_and_permission_statuses_are_enforced_at_the_wire() {
    let server = TestServer::start().await;
    let mut client = server.client().await;
    let independent = client
        .seed_game(SeedGame {
            game_id: "independent".into(),
            black_user_id: "new-black".into(),
            white_user_id: "new-white".into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(independent.black_user_id, "new-black");
    assert_eq!(independent.white_user_id, "new-white");
    let initial = create(&mut client, "binding").await;
    assert_eq!(initial.black_user_id, "black");
    assert_eq!(initial.white_user_id, "white");
    let first = command("binding", "first", 0, proto::Player::Black, "7g7f");
    client.make_move(first.clone()).await.unwrap();
    let mut outsider = first;
    outsider.user_id = "outsider".into();
    assert_eq!(
        client.make_move(outsider).await.unwrap_err().code(),
        Code::PermissionDenied
    );
    let mut outsider = resign("binding", "first", 0, proto::Player::White);
    outsider.user_id = "outsider".into();
    assert_eq!(
        client.resign(outsider).await.unwrap_err().code(),
        Code::PermissionDenied
    );
    drop(client);
    server.shutdown().await;
}
}
