crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実 user HTTP・PostgreSQL・gRPC・Valkey と WebSocket ハンドシェイク。未知 token の本人確認を含む。",
        reason: "存在しないが形式の正しい token は実 user HTTP に問い合わせて 401 となり、欠落・不正・重複 Cookie もハンドシェイクで拒否する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "browser-headers", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "Missing/evil/duplicate Origin, cross-site metadata and absent/wrong protocol reject with 403." }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "session-auth", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "Missing/duplicate/invalid/unknown session cookies reject upgrade with 401." }],
    };
#[tokio::test]
async fn handshake_requires_exact_origin_unique_session_and_supported_subprotocol() {
    let services = Services::start().await;
    let mut requests = Vec::new();
    let mut request = services.request(&services.black);
    request.headers_mut().remove("origin");
    requests.push((request, StatusCode::FORBIDDEN));
    let mut request = services.request(&services.black);
    request
        .headers_mut()
        .insert("origin", HeaderValue::from_static("https://evil.example"));
    requests.push((request, StatusCode::FORBIDDEN));
    let mut request = services.request(&services.black);
    request
        .headers_mut()
        .append("origin", services.origin.parse().unwrap());
    requests.push((request, StatusCode::FORBIDDEN));
    let mut request = services.request(&services.black);
    request
        .headers_mut()
        .insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
    requests.push((request, StatusCode::FORBIDDEN));
    for protocol in [None, Some("shogi.realtime.v0")] {
        let mut request = services.request(&services.black);
        request.headers_mut().remove("sec-websocket-protocol");
        if let Some(protocol) = protocol {
            request
                .headers_mut()
                .insert("sec-websocket-protocol", protocol.parse().unwrap());
        }
        requests.push((request, StatusCode::FORBIDDEN));
    }
    let mut request = services.request(&services.black);
    request.headers_mut().remove("cookie");
    requests.push((request, StatusCode::UNAUTHORIZED));
    for cookie in [
        format!("{}; {}", services.black.cookie(), services.black.cookie()),
        "user_session=invalid".into(),
        format!("user_session={}", "0".repeat(64)),
    ] {
        let mut request = services.request(&services.black);
        request
            .headers_mut()
            .insert("cookie", cookie.parse().unwrap());
        requests.push((request, StatusCode::UNAUTHORIZED));
    }
    let mut request = services.request(&services.black);
    request
        .headers_mut()
        .append("cookie", services.black.cookie().parse().unwrap());
    requests.push((request, StatusCode::UNAUTHORIZED));
    for (request, expected) in requests {
        assert_eq!(rejected(request).await.0, expected);
    }
    services.stop().await;
}
}
