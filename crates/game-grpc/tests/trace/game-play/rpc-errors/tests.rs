use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Generated gRPC client over TCP, GameRpc, application and real PostgreSQL; test-only game seeding is fixture setup.",
        reason: "NotFound, wrong-turn/illegal FailedPrecondition, stale Aborted metadata, request correction and post-finish rejection are asserted with unchanged state.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn statuses_sequence_metadata_and_rejected_request_correction() {
    let server = TestServer::start().await;
    let mut client = server.client().await;
    assert_eq!(
        client
            .get_snapshot(proto::GetSnapshotRequest {
                game_id: "absent".into()
            })
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
    assert_eq!(
        client
            .make_move(command("absent", "m", 0, proto::Player::Black, "7g7f"))
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
    assert_eq!(
        client
            .resign(resign("absent", "r", 0, proto::Player::White))
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
    let initial = create(&mut client, "errors").await;
    assert_eq!(
        client
            .seed_game(SeedGame {
                game_id: "errors".into(),
                black_user_id: "black".into(),
                white_user_id: "white".into()
            })
            .await
            .unwrap_err()
            .code(),
        Code::AlreadyExists
    );
    for request in [
        command("errors", "retry", 0, proto::Player::White, "3c3d"),
        command("errors", "retry", 0, proto::Player::Black, "7g7e"),
    ] {
        assert_eq!(
            client.make_move(request).await.unwrap_err().code(),
            Code::FailedPrecondition
        );
        assert_eq!(snapshot(&mut client, "errors").await, initial);
    }
    let stale = client
        .make_move(command("errors", "retry", 5, proto::Player::Black, "7g7f"))
        .await
        .unwrap_err();
    assert_eq!(stale.code(), Code::Aborted);
    assert_eq!(stale.metadata().get("actual-seq").unwrap(), "0");
    assert!(stale.message().contains("expected 5, actual 0"));
    let accepted = client
        .make_move(command("errors", "retry", 0, proto::Player::Black, "7g7f"))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(accepted.seq, 1);
    let stale = client
        .resign(resign("errors", "end", 0, proto::Player::White))
        .await
        .unwrap_err();
    assert_eq!(stale.code(), Code::Aborted);
    assert_eq!(stale.metadata().get("actual-seq").unwrap(), "1");
    client
        .resign(resign("errors", "end", 1, proto::Player::White))
        .await
        .unwrap();
    let finished = snapshot(&mut client, "errors").await;
    assert_eq!(
        client
            .make_move(command("errors", "after", 2, proto::Player::White, "3c3d"))
            .await
            .unwrap_err()
            .code(),
        Code::FailedPrecondition
    );
    assert_eq!(
        client
            .resign(resign("errors", "after", 2, proto::Player::Black))
            .await
            .unwrap_err()
            .code(),
        Code::FailedPrecondition
    );
    assert_eq!(snapshot(&mut client, "errors").await, finished);
    drop(client);
    server.shutdown().await;
}
}
