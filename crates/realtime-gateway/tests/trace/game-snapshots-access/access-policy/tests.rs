use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.",
        reason: "Private outsider and missing access RPCs are NotFound; participant access succeeds and existing public spectator closes after privacy changes.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "game-snapshots-access", contract: "viewer-read", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "Private outsider and absent viewer snapshot are NotFound while participant snapshot succeeds." }, crate::trace_support::Evidence { feature: "game-snapshots-access", contract: "grpc-viewer", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "Generated TCP RPCs assert both private/missing errors and participant success through real gateway fixture services." }, crate::trace_support::Evidence { feature: "game-snapshots-access", contract: "durable-read", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Exercises, reason: "Viewer read exercises the private/participant access paths." }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "upgrade-permits", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "Private outsider and absent game yield indistinguishable 404 upgrade status/body." }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "command-input", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "Forged user_id returns invalid_input with sequence still zero; spectator move/resign return not_participant." }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "command-response", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "Outsider move/resign produce not_participant." }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "live-checks", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "Spectator cannot mutate or supply a participant identity." }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "outgoing", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "Privacy revoked for existing spectator closes on the next event." }],
    };
#[tokio::test]
async fn private_games_are_hidden_and_spectators_cannot_mutate_or_spoof_a_participant() {
    let mut services = Services::start().await;
    let mut viewer = services.connect(&services.viewer).await;
    initial(&mut viewer, &services.game, "0").await;
    send(&mut viewer, movement("outsider", "0", "7g7f")).await;
    assert_eq!(receive(&mut viewer).await["code"], "not_participant");
    send(
        &mut viewer,
        json!({"type":"resign", "request_id":"outsider-resign", "expected_seq":"0"}),
    )
    .await;
    assert_eq!(receive(&mut viewer).await["code"], "not_participant");
    let mut forged = movement("forged", "0", "7g7f");
    forged["user_id"] = services.black.id.clone().into();
    send(&mut viewer, forged).await;
    assert_eq!(receive(&mut viewer).await["code"], "invalid_input");
    assert_eq!(services.snapshot().await.seq, 0);
    services.visibility("private").await;
    let denied = proto::ViewerRequest {
        game_id: services.game.clone(),
        viewer_user_id: services.viewer.id.clone(),
    };
    let missing = proto::ViewerRequest {
        game_id: uuid::Uuid::new_v4().to_string(),
        viewer_user_id: services.viewer.id.clone(),
    };
    for request in [denied, missing] {
        assert_eq!(
            services
                .client
                .get_snapshot_for_viewer(request.clone())
                .await
                .unwrap_err()
                .code(),
            tonic::Code::NotFound
        );
        assert_eq!(
            services
                .client
                .check_view_access(request)
                .await
                .unwrap_err()
                .code(),
            tonic::Code::NotFound
        );
    }
    let participant = proto::ViewerRequest {
        game_id: services.game.clone(),
        viewer_user_id: services.black.id.clone(),
    };
    assert_eq!(
        services
            .client
            .get_snapshot_for_viewer(participant.clone())
            .await
            .unwrap()
            .into_inner()
            .seq,
        0
    );
    services
        .client
        .check_view_access(participant)
        .await
        .unwrap();
    let hidden = rejected(services.request(&services.viewer)).await;
    let mut absent = services.request(&services.viewer);
    *absent.uri_mut() = format!(
        "{}/ws/games/{}",
        services.origin.replacen("http", "ws", 1),
        uuid::Uuid::new_v4()
    )
    .parse()
    .unwrap();
    assert_eq!(hidden, rejected(absent).await);
    assert_eq!(hidden.0, StatusCode::NOT_FOUND);
    let mut black = services.connect(&services.black).await;
    initial(&mut black, &services.game, "0").await;
    send(&mut black, movement("participant", "0", "7g7f")).await;
    assert_eq!(receive(&mut black).await["type"], "accepted");
    services.publish().await;
    closed(&mut viewer).await;
    services.stop().await;
}
}
