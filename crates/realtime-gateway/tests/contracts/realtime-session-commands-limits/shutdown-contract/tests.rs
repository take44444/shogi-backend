use super::*;

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
