use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。 スナップショットのバイト数の上限を1にする。",
        reason: "スナップショットのバイト数の上限を1にすると、不完全な局面を送らずに接続を閉じる。",
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
