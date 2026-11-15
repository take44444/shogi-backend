crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture. Calls Gateway::shutdown before any connection.",
        reason: "The first upgrade after shutdown is rejected 503.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "upgrade-permits", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture. Calls Gateway::shutdown before any connection.", relationship: crate::trace_support::Relationship::Asserts, reason: "Shutdown before any connection makes first upgrade 503." }],
    };
#[tokio::test]
async fn shutdown_before_any_connection_refuses_the_first_upgrade() {
    let services = Services::start().await;
    services.gateway.shutdown();
    assert_eq!(
        rejected(services.request(&services.viewer)).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    services.stop().await;
}
}
