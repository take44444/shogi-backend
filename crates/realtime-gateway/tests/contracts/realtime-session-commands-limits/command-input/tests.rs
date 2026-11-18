use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "realtime-gateway/frontend-ws-upgrade", relationship: crate::trace_support::Relationship::Asserts, scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。 入力・頻度の上限を変更し、永続化されたスナップショットを検証する。", reason: "入力バイト数の上限128と頻度の上限1で、過剰な入力を送るソケットを終了する。永続化された連番は0のままとなる。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["realtime-gateway"], fake: &[] }] };
#[tokio::test]
async fn oversized_frames_and_input_rate_close_without_committing_commands() {
    let mut services = Services::with_limits(Limits {
        input_bytes: 128,
        messages_per_second: 1,
        ..Limits::default()
    })
    .await;
    let mut oversized = services.connect(&services.black).await;
    initial(&mut oversized, &services.game, "0").await;
    send(&mut oversized, json!({"padding":"x".repeat(1024)})).await;
    closed(&mut oversized).await;
    let mut rate = services.connect(&services.white).await;
    initial(&mut rate, &services.game, "0").await;
    for _ in 0..32 {
        if rate.send(Message::Text("{}".into())).await.is_err() {
            break;
        }
    }
    timeout(WAIT, async {
        loop {
            match rate.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                Some(Ok(Message::Text(raw))) => {
                    let message: Value = serde_json::from_str(&raw).unwrap();
                    assert!(
                        message["type"] == "resync_required"
                            || (message["type"] == "error" && message["code"] == "invalid_input")
                    );
                }
                other => panic!("unexpected response to rate limit {other:?}"),
            }
        }
    })
    .await
    .expect("rate limit closes connection");
    assert_eq!(services.snapshot().await.seq, 0);
    services.stop().await;
}
}
