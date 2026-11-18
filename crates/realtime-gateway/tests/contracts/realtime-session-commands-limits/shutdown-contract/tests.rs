use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "realtime-gateway/frontend-ws-upgrade", relationship: crate::trace_support::Relationship::Asserts, scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。 Gateway::shutdownを直接呼ぶ。", reason: "接続中のソケットが閉じ、その後のアップグレードが503を返す。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["realtime-gateway"], fake: &[] }] };
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

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "realtime-gateway/frontend-ws-upgrade", relationship: crate::trace_support::Relationship::Asserts, scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。 一度も接続を確立する前にGateway::shutdownを呼ぶ。", reason: "停止後の最初のアップグレードを503で拒否する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["realtime-gateway"], fake: &[] }] };
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
