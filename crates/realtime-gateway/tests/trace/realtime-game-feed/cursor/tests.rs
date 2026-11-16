use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture. Publishes controlled invalid/gapped notifications directly.",
        reason: "Real malformed schema and gapped published event terminate the WebSocket.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "realtime-game-feed", contract: "subscription-pump", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture. Publishes controlled invalid/gapped notifications directly.", relationship: crate::trace_support::Relationship::Asserts, reason: "WebSocket closes on malformed schema or gap, exercising the pump and asserting the connection-level failure response." }],
    };
#[tokio::test]
async fn malformed_or_gapped_broker_notifications_require_a_new_connection() {
    let services = Services::start().await;
    let game = GameId::new(services.game.clone()).unwrap();
    let mut socket = services.connect(&services.viewer).await;
    initial(&mut socket, &services.game, "0").await;
    let mut publisher = ValkeyEventPublisher::connect(&services.valkey_url)
        .await
        .unwrap();
    publisher
        .publish(&CommittedEvent {
            game_id: game.clone(),
            request_id: RequestId::new("missing-sequence").unwrap(),
            user_id: UserId::new(services.black.id.clone()).unwrap(),
            seq: 2,
            event: DomainEvent::MoveCommitted {
                movement: "7g7f".parse().unwrap(),
                in_check: false,
                result: None,
            },
        })
        .await
        .unwrap();
    closed(&mut socket).await;
    let mut socket = services.connect(&services.viewer).await;
    initial(&mut socket, &services.game, "0").await;
    let mut connection = redis::Client::open(services.valkey_url.clone())
        .unwrap()
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    let subscribers: i64 = redis::cmd("PUBLISH")
        .arg(game_channel(&game))
        .arg(r#"{"schema_version":999}"#)
        .query_async(&mut connection)
        .await
        .unwrap();
    assert!(subscribers >= 1);
    closed(&mut socket).await;
    services.stop().await;
}
}
