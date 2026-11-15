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
