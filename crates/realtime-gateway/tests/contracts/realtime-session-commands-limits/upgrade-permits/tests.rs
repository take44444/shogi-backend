use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "realtime-gateway/frontend-ws-upgrade", relationship: crate::trace_support::Relationship::Asserts, scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。 接続全体・ユーザーごとの上限を変更する。", reason: "接続全体の上限2・ユーザーごとの上限1を超える接続を429で拒否し、切断後は再接続できる。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["realtime-gateway"], fake: &[] }] };
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
