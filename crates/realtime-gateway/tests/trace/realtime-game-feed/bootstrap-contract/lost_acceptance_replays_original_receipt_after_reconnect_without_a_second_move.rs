crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture. Drops connection after durable acceptance and retries original identity.",
        reason: "Reconnect snapshot is seq 1 and retry returns original accepted seq 1 with one persisted event.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "command-response", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "Retry after reconnect returns original accepted sequence without duplicate event." }],
    };
#[tokio::test]
async fn lost_acceptance_replays_original_receipt_after_reconnect_without_a_second_move() {
    let mut services = Services::start().await;
    let mut black = services.connect(&services.black).await;
    initial(&mut black, &services.game, "0").await;
    let command = movement("lost-response", "0", "7g7f");
    send(&mut black, command.clone()).await;
    timeout(WAIT, async {
        while services.snapshot().await.seq != 1 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    drop(black);
    let mut retry = services.connect(&services.black).await;
    initial(&mut retry, &services.game, "1").await;
    send(&mut retry, command).await;
    let accepted = receive(&mut retry).await;
    assert_eq!(accepted["type"], "accepted");
    assert_eq!(accepted["request_id"], "lost-response");
    assert_eq!(accepted["seq"], "1");
    assert_eq!(services.snapshot().await.seq, 1);
    services.publish().await;
    assert!(
        !services.publisher.publish_next().await.unwrap(),
        "retry does not enqueue another event"
    );
    services.stop().await;
}
}
