crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture. Custom input/rate limits and durable snapshot assertion.",
        reason: "Input byte limit 128 and rate one terminate abusive sockets while durable sequence remains zero.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "upgrade-permits", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture. Custom input/rate limits and durable snapshot assertion.", relationship: crate::trace_support::Relationship::Asserts, reason: "Oversized frames close without committing." }],
    };
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
