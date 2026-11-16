use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture. Custom total/per-user limits.",
        reason: "Total two/per-user one rejects excess with 429, and reconnect succeeds after disconnect.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn connection_and_user_limits_release_capacity_after_disconnect() {
    let services = Services::with_limits(Limits {
        connections: 2,
        user_connections: 1,
        ..Limits::default()
    })
    .await;
    let mut viewer = services.connect(&services.viewer).await;
    initial(&mut viewer, &services.game, "0").await;
    assert_eq!(
        rejected(services.request(&services.viewer)).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    let mut black = services.connect(&services.black).await;
    initial(&mut black, &services.game, "0").await;
    assert_eq!(
        rejected(services.request(&services.white)).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    drop(viewer);
    let mut white = connect_after_release(&services, &services.white).await;
    initial(&mut white, &services.game, "0").await;
    drop(black);
    let mut black = connect_after_release(&services, &services.black).await;
    initial(&mut black, &services.game, "0").await;
    services.stop().await;
}
}
