crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture. Calls Gateway::shutdown directly.",
        reason: "Active socket closes and later upgrade returns 503.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn shutdown_closes_active_connections_and_refuses_new_upgrades() {
    let services = Services::start().await;
    let mut socket = services.connect(&services.viewer).await;
    initial(&mut socket, &services.game, "0").await;
    services.gateway.shutdown();
    closed(&mut socket).await;
    assert_eq!(
        rejected(services.request(&services.viewer)).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    services.stop().await;
}
}
