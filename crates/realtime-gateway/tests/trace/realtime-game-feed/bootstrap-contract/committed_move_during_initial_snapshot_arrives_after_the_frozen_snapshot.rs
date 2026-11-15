crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture. SnapshotHold freezes initial response while a real subscriber receives a commit.",
        reason: "Holds frozen seq-0 snapshot, verifies real subscriber, commits/publishes seq 1, then receives snapshot 0 followed by event 1.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "realtime-game-feed", contract: "snapshot-frame", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture. SnapshotHold freezes initial response while a real subscriber receives a commit.", relationship: crate::trace_support::Relationship::Asserts, reason: "Snapshot sequence zero is observed before the later move frame." }],
    };
#[tokio::test]
async fn committed_move_during_initial_snapshot_arrives_after_the_frozen_snapshot() {
    let (captured, snapshot_ready) = oneshot::channel();
    let (release_snapshot, release) = oneshot::channel();
    let mut services =
        Services::with_snapshot_hold(Limits::default(), Some(SnapshotHold { captured, release }))
            .await;
    let mut viewer = services.connect(&services.viewer).await;
    assert_eq!(timeout(WAIT, snapshot_ready).await.unwrap().unwrap(), 0);
    let mut broker = redis::Client::open(services.valkey_url.clone())
        .unwrap()
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    let game = GameId::new(services.game.clone()).unwrap();
    let (_, subscribers): (String, i64) = redis::cmd("PUBSUB")
        .arg("NUMSUB")
        .arg(game_channel(&game))
        .query_async(&mut broker)
        .await
        .unwrap();
    assert_eq!(
        subscribers, 1,
        "subscription is active while snapshot is held"
    );
    let accepted = services
        .client
        .make_move(proto::MakeMoveRequest {
            game_id: services.game.clone(),
            request_id: "during-bootstrap".into(),
            expected_seq: Some(0),
            usi: "7g7f".into(),
            user_id: services.black.id.clone(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(accepted.seq, 1);
    services.publish().await;
    assert_eq!(services.snapshot().await.seq, 1);
    release_snapshot.send(()).unwrap();
    initial(&mut viewer, &services.game, "0").await;
    let event = receive(&mut viewer).await;
    assert_eq!(event["type"], "event");
    assert_eq!(event["kind"], "move_committed");
    assert_eq!(event["seq"], "1");
    assert_eq!(event["usi"], "7g7f");
    services.stop().await;
}
}
