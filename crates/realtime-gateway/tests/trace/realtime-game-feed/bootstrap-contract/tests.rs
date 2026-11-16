use super::*;

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

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.",
        reason: "Three real WebSockets receive initial snapshot, two committed moves and resignation.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "realtime-game-feed", contract: "snapshot-frame", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "All connected participants and spectator receive snapshot frames with expected game_id/sequence." }, crate::trace_support::Evidence { feature: "realtime-game-feed", contract: "event-frame", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "Players and spectator assert move/resignation kinds, USI/results, and absence of user_id/request_id." }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "command-input", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Exercises, reason: "Valid participant move and resign commands dispatch and return matching acceptance." }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "command-response", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "Accepted request identity and sequence match participant commands." }],
    };
#[tokio::test]
async fn participants_and_spectator_receive_committed_moves_and_resignation() {
    let mut services = Services::start().await;
    let mut black = services.connect(&services.black).await;
    let mut white = services.connect(&services.white).await;
    let mut viewer = services.connect(&services.viewer).await;
    for socket in [&mut black, &mut white, &mut viewer] {
        initial(socket, &services.game, "0").await;
    }
    send(&mut black, movement("black-move", "0", "7g7f")).await;
    let accepted = receive(&mut black).await;
    assert_eq!(accepted["type"], "accepted");
    assert_eq!(accepted["request_id"], "black-move");
    assert_eq!(accepted["seq"], "1");
    services.publish().await;
    for socket in [&mut black, &mut white, &mut viewer] {
        let event = receive(socket).await;
        assert_eq!(event["type"], "event");
        assert_eq!(event["kind"], "move_committed");
        assert_eq!(event["seq"], "1");
        assert_eq!(event["usi"], "7g7f");
        assert!(event.get("request_id").is_none());
        assert!(event.get("user_id").is_none());
    }
    send(&mut white, movement("white-move", "1", "3c3d")).await;
    assert_eq!(receive(&mut white).await["seq"], "2");
    services.publish().await;
    for socket in [&mut black, &mut white, &mut viewer] {
        assert_eq!(receive(socket).await["usi"], "3c3d");
    }
    send(
        &mut white,
        json!({"type":"resign", "request_id":"end", "expected_seq":"2"}),
    )
    .await;
    assert_eq!(receive(&mut white).await["type"], "accepted");
    services.publish().await;
    for socket in [&mut black, &mut white, &mut viewer] {
        let event = receive(socket).await;
        assert_eq!(event["seq"], "3");
        assert_eq!(event["kind"], "game_finished");
        assert_eq!(
            event["result"],
            json!({"winner":"black", "reason":"resignation"})
        );
    }
    assert_eq!(services.snapshot().await.seq, 3);
    services.stop().await;
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture. Drops connection after durable acceptance and retries original identity.",
        reason: "Reconnect snapshot is seq 1 and retry returns original accepted seq 1 with one persisted event.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "command-response", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "Retry after reconnect returns original accepted sequence without duplicate event." }],
    };
#[tokio::test]
async fn lost_acceptance_replays_original_receipt_after_reconnect_without_a_second_move() {
    let mut services = Services::start().await;
    let mut black = services.connect(&services.black).await;
    initial(&mut black, &services.game, "0").await;
    let command = movement("lost-response", "0", "7g7f");
    send(&mut black, command.clone()).await;
    timeout(WAIT, async {
        while services.snapshot().await.seq != 1 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    drop(black);
    let mut retry = services.connect(&services.black).await;
    initial(&mut retry, &services.game, "1").await;
    send(&mut retry, command).await;
    let accepted = receive(&mut retry).await;
    assert_eq!(accepted["type"], "accepted");
    assert_eq!(accepted["request_id"], "lost-response");
    assert_eq!(accepted["seq"], "1");
    assert_eq!(services.snapshot().await.seq, 1);
    services.publish().await;
    assert!(
        !services.publisher.publish_next().await.unwrap(),
        "retry does not enqueue another event"
    );
    services.stop().await;
}
}
