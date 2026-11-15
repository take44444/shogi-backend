crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.",
        reason: "Anonymous current is 401, foreign owner is 404 and wrong Origin/Host is 403.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "reads", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "Browser retrieves current playing game and stored refusal by ticket identity." }, crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "heartbeat", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "HTTP rejects sequence zero and duplicate sequence one returns the same ticket." }, crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "rpc-boundary", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Exercises, reason: "All five RPCs are exercised through browser calls and return the asserted current, stored, heartbeat and cancellation outcomes." }, crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "browser-response", scope: "Real HTTP/WebSocket gateway, user HTTP service, game gRPC service, PostgreSQL and Valkey; external OIDC provider is a fixture.", relationship: crate::trace_support::Relationship::Asserts, reason: "New join is 201 with no-store, replay is 200 with identical JSON, pool conflict is 409, playing cancellation is 409 already_matched with game_id." }],
    };
#[tokio::test]
async fn authenticated_matchmaking_recovers_requests_and_rejects_cross_site_or_foreign_access() {
    use game_application::{matchmaking::MatchmakingRepository, rating::Pool};
    let services = Services::start().await;
    let http = reqwest::Client::new();
    let ticket = uuid::Uuid::new_v4().to_string();
    let endpoint = format!("{}/matchmaking/tickets/{ticket}", services.origin);
    assert_eq!(
        http.get(format!("{}/matchmaking/me", services.origin))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        http.put(&endpoint)
            .header("cookie", services.viewer.cookie())
            .header("origin", "http://attacker.invalid")
            .json(&json!({"pool":"ten_minute_30"}))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::FORBIDDEN
    );
    assert_eq!(
        http.put(&endpoint)
            .header("cookie", services.viewer.cookie())
            .header("origin", &services.origin)
            .header("host", "attacker.invalid")
            .json(&json!({"pool":"ten_minute_30"}))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::FORBIDDEN
    );
    let response = http
        .put(&endpoint)
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .json(&json!({"pool":"ten_minute_30"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let first: Value = response.json().await.unwrap();
    let conflict = http
        .put(&endpoint)
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .json(&json!({"pool":"byoyomi_30"}))
        .send()
        .await
        .unwrap();
    assert_eq!(conflict.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(
        conflict.json::<Value>().await.unwrap()["code"],
        "pool_conflict"
    );
    let retry = http
        .put(&endpoint)
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .json(&json!({"pool":"ten_minute_30"}))
        .send()
        .await
        .unwrap();
    assert_eq!(retry.status(), reqwest::StatusCode::OK);
    assert_eq!(retry.json::<Value>().await.unwrap(), first);
    assert_eq!(
        http.get(&endpoint)
            .header("cookie", services.black.cookie())
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::NOT_FOUND
    );
    let refused_endpoint = format!(
        "{}/matchmaking/tickets/{}",
        services.origin,
        uuid::Uuid::new_v4()
    );
    let refused = http
        .put(&refused_endpoint)
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .json(&json!({"pool":"byoyomi_30"}))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), reqwest::StatusCode::CONFLICT);
    let refused: Value = refused.json().await.unwrap();
    assert_eq!(refused["code"], "already_waiting");
    let lookup = http
        .get(&refused_endpoint)
        .header("cookie", services.viewer.cookie())
        .send()
        .await
        .unwrap();
    assert_eq!(lookup.status(), reqwest::StatusCode::OK);
    assert_eq!(lookup.json::<Value>().await.unwrap(), refused);
    assert_eq!(
        http.post(format!("{endpoint}/heartbeat"))
            .header("cookie", services.viewer.cookie())
            .header("origin", &services.origin)
            .json(&json!({"sequence":0}))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::BAD_REQUEST
    );
    let heartbeat = http
        .post(format!("{endpoint}/heartbeat"))
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .json(&json!({"sequence":1}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let duplicate = http
        .post(format!("{endpoint}/heartbeat"))
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .json(&json!({"sequence":1}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(heartbeat, duplicate);
    let cancelled = http
        .delete(&endpoint)
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .send()
        .await
        .unwrap();
    assert_eq!(cancelled.status(), reqwest::StatusCode::OK);
    assert_eq!(
        cancelled.json::<Value>().await.unwrap()["state"],
        "cancelled"
    );
    let retry = http
        .put(&refused_endpoint)
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .json(&json!({"pool":"byoyomi_30"}))
        .send()
        .await
        .unwrap();
    assert_eq!(retry.status(), reqwest::StatusCode::CONFLICT);
    let early_endpoint = format!(
        "{}/matchmaking/tickets/{}",
        services.origin,
        uuid::Uuid::new_v4()
    );
    http.delete(&early_endpoint)
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let early = http
        .put(&early_endpoint)
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .json(&json!({"pool":"byoyomi_30"}))
        .send()
        .await
        .unwrap();
    assert_eq!(early.status(), reqwest::StatusCode::OK);
    assert_eq!(early.json::<Value>().await.unwrap()["state"], "cancelled");
    let opponent = login(&services.accounts, "opponent").await;
    let mut endpoints = Vec::new();
    for browser in [&services.viewer, &opponent] {
        let endpoint = format!(
            "{}/matchmaking/tickets/{}",
            services.origin,
            uuid::Uuid::new_v4()
        );
        assert_eq!(
            http.put(&endpoint)
                .header("cookie", browser.cookie())
                .header("origin", &services.origin)
                .json(&json!({"pool":"byoyomi_30"}))
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::CREATED
        );
        endpoints.push(endpoint);
    }
    let game = services.games.tick(Pool::Byoyomi30).await.unwrap().unwrap();
    let current = http
        .get(format!("{}/matchmaking/me", services.origin))
        .header("cookie", services.viewer.cookie())
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(current["state"], "playing");
    assert_eq!(current["ticket"]["game_id"], game.as_str());
    let cancel = http
        .delete(&endpoints[0])
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .send()
        .await
        .unwrap();
    assert_eq!(cancel.status(), reqwest::StatusCode::CONFLICT);
    let result: Value = cancel.json().await.unwrap();
    assert_eq!(result["code"], "already_matched");
    assert_eq!(result["game_id"], game.as_str());
    services.stop().await;
}
}
