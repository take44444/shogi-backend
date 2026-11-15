crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実サービス群と WebSocket。HTTP logout と SQL による期限切れを起こし、次のコマンド・通知の拒否を確認する。",
        reason: "有効な user HTTP セッションで WebSocket 接続する。",
        relationship: crate::trace_support::Relationship::Exercises,
        also: &[crate::trace_support::Evidence { feature: "account-session", contract: "realtime-recheck", scope: "実サービス群と WebSocket。HTTP logout と SQL による期限切れを起こし、次のコマンド・通知の拒否を確認する。", relationship: crate::trace_support::Relationship::Asserts, reason: "実 HTTP logout の後のコマンドで接続が閉じ game seq が変わらず、DB で期限切れにした viewer は次の通知で閉じ、双方の再接続も 401 となることを確認する。" }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "session-auth", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture. Real account logout and forced session expiry.", relationship: crate::trace_support::Relationship::Asserts, reason: "Actual logout before a command and expiry before a notification close sockets; subsequent upgrades are 401." }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "live-checks", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture. Real account logout and forced session expiry.", relationship: crate::trace_support::Relationship::Asserts, reason: "Logout before sending move closes and leaves durable sequence zero." }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "outgoing", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture. Real account logout and forced session expiry.", relationship: crate::trace_support::Relationship::Asserts, reason: "Expired viewer closes before published resignation delivery." }],
    };
#[tokio::test]
async fn logout_closes_before_command_and_expiry_closes_before_notification() {
    let mut services = Services::start().await;
    let mut black = services.connect(&services.black).await;
    let mut viewer = services.connect(&services.viewer).await;
    initial(&mut black, &services.game, "0").await;
    initial(&mut viewer, &services.game, "0").await;
    let response = reqwest::Client::new()
        .post(format!("{}/auth/logout", services.user_url))
        .header("origin", &services.origin)
        .header("cookie", services.black.cookie())
        .timeout(WAIT)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    send(&mut black, movement("after-logout", "0", "7g7f")).await;
    closed(&mut black).await;
    assert_eq!(services.snapshot().await.seq, 0);
    assert_eq!(
        rejected(services.request(&services.black)).await.0,
        StatusCode::UNAUTHORIZED
    );
    let db = Database::connect(services.user_database.url())
        .await
        .unwrap();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE sessions SET expires_at=$1 WHERE token_hash=$2",
        [now().into(), hash(&services.viewer.token).into()],
    ))
    .await
    .unwrap();
    db.close().await.unwrap();
    services
        .client
        .resign(proto::ResignRequest {
            game_id: services.game.clone(),
            request_id: "internal-resign".into(),
            expected_seq: Some(0),
            user_id: services.white.id.clone(),
        })
        .await
        .unwrap();
    services.publish().await;
    closed(&mut viewer).await;
    assert_eq!(
        rejected(services.request(&services.viewer)).await.0,
        StatusCode::UNAUTHORIZED
    );
    services.stop().await;
}
}
