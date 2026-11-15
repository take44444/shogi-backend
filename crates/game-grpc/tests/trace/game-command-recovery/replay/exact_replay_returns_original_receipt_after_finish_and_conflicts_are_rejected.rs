crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Generated gRPC client over TCP, GameRpc, application and real PostgreSQL; test-only game seeding is fixture setup.",
        reason: "Exact move/resign retries after finish return original acceptances; changed sequence/user/move/action conflict with no extra advance.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn exact_replay_returns_original_receipt_after_finish_and_conflicts_are_rejected() {
    let server = TestServer::start().await;
    let mut client = server.client().await;
    create(&mut client, "replay").await;
    let first = command("replay", "first", 0, proto::Player::Black, "7g7f");
    let receipt = client.make_move(first.clone()).await.unwrap().into_inner();
    client
        .make_move(command("replay", "second", 1, proto::Player::White, "3c3d"))
        .await
        .unwrap();
    let resignation = resign("replay", "end", 2, proto::Player::White);
    let finish = client
        .resign(resignation.clone())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(client.make_move(first).await.unwrap().into_inner(), receipt);
    assert_eq!(
        client.resign(resignation).await.unwrap().into_inner(),
        finish
    );
    for changed in [
        command("replay", "first", 1, proto::Player::Black, "7g7f"),
        command("replay", "first", 0, proto::Player::White, "7g7f"),
        command("replay", "first", 0, proto::Player::Black, "2g2f"),
        command("replay", "end", 2, proto::Player::White, "3c3d"),
    ] {
        assert_eq!(
            client.make_move(changed).await.unwrap_err().code(),
            Code::AlreadyExists
        );
    }
    assert_eq!(
        client
            .resign(resign("replay", "first", 0, proto::Player::Black))
            .await
            .unwrap_err()
            .code(),
        Code::AlreadyExists
    );
    assert_eq!(snapshot(&mut client, "replay").await.seq, 3);
    drop(client);
    server.shutdown().await;
}
}
