use super::*;

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
