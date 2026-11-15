crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture. Snapshot bytes limit one.",
        reason: "Snapshot bytes limit one closes without sending a partial position.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn snapshot_size_limit_closes_without_sending_a_partial_position() {
    let services = Services::with_limits(Limits {
        snapshot_bytes: 1,
        ..Limits::default()
    })
    .await;
    let mut socket = services.connect(&services.viewer).await;
    closed(&mut socket).await;
    services.stop().await;
}
}
