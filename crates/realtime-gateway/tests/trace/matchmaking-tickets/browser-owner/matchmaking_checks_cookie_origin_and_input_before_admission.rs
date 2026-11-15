crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.",
        reason: "Missing mutation Origin, duplicate cookie, unknown pool and forged user_id are rejected; no rejected admission leaves a ticket.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "browser-response", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "Missing-Origin mutation responses include no-store." }],
    };
#[tokio::test]
async fn matchmaking_checks_cookie_origin_and_input_before_admission() {
    let services = Services::start().await;
    let http = reqwest::Client::new();
    let endpoint = format!(
        "{}/matchmaking/tickets/{}",
        services.origin,
        uuid::Uuid::new_v4()
    );
    for request in [
        http.put(&endpoint)
            .header("cookie", services.viewer.cookie())
            .json(&json!({"pool":"ten_minute_30"})),
        http.delete(&endpoint)
            .header("cookie", services.viewer.cookie()),
        http.post(format!("{endpoint}/heartbeat"))
            .header("cookie", services.viewer.cookie())
            .json(&json!({"sequence":1})),
    ] {
        let response = request.send().await.unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    assert_eq!(
        http.get(&endpoint)
            .header(
                "cookie",
                format!("{}; {}", services.viewer.cookie(), services.viewer.cookie())
            )
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    for body in [
        json!({"pool":"unknown"}),
        json!({"pool":"ten_minute_30","user_id":services.black.id}),
    ] {
        assert_eq!(
            http.put(&endpoint)
                .header("cookie", services.viewer.cookie())
                .header("origin", &services.origin)
                .json(&body)
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        http.get(&endpoint)
            .header("cookie", services.viewer.cookie())
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::NOT_FOUND
    );
    let current = http
        .get(format!("{}/matchmaking/me", services.origin))
        .header("cookie", services.viewer.cookie())
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(current, json!({"state":"idle","ticket":null}));
    services.stop().await;
}
}
