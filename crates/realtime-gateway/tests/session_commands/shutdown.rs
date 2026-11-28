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

#[tokio::test]
async fn shutdown_during_snapshot_releases_connection_capacity_without_snapshot_data() {
    let (captured, snapshot_ready) = oneshot::channel();
    let (release_snapshot, release) = oneshot::channel();
    let services = Services::with_snapshot_hold(
        Limits {
            connections: 1,
            user_connections: 1,
            ..Limits::default()
        },
        Some(SnapshotHold { captured, release }),
    )
    .await;
    let mut socket = services.connect(&services.viewer).await;
    timeout(WAIT, snapshot_ready).await.unwrap().unwrap();
    services.gateway.shutdown();
    closed(&mut socket).await;
    assert!(services.gateway.wait_closed(WAIT).await);
    let _ = release_snapshot.send(());
    services.stop().await;
}
