use super::*;

#[tokio::test]
async fn authenticated_matchmaking_recovers_requests_and_rejects_cross_site_or_foreign_access() {
    use game_application::{matchmaking::MatchmakingRepository, rating::Pool};
    let services = Services::start().await;
    let http = reqwest::Client::new();
    let ticket = uuid::Uuid::new_v4().to_string();
    let endpoint = format!("{}/matchmaking/tickets/{ticket}", services.origin);
    assert_eq!(
        http.get(&endpoint).send().await.unwrap().status(),
        reqwest::StatusCode::METHOD_NOT_ALLOWED
    );
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
    let waiting_ready = http
        .post(format!("{endpoint}/ready"))
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .send()
        .await
        .unwrap();
    assert_eq!(waiting_ready.status(), reqwest::StatusCode::OK);
    assert_eq!(waiting_ready.json::<Value>().await.unwrap(), first);
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
        http.post(format!("{endpoint}/heartbeat"))
            .header("cookie", services.black.cookie())
            .header("origin", &services.origin)
            .json(&json!({"sequence":1}))
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
    let recovered_refusal = http
        .put(&refused_endpoint)
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .json(&json!({"pool":"byoyomi_30"}))
        .send()
        .await
        .unwrap();
    assert_eq!(recovered_refusal.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(recovered_refusal.json::<Value>().await.unwrap(), refused);
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
    assert_eq!(heartbeat["heartbeat_sequence"], 1);
    assert_eq!(heartbeat, duplicate);
    let progressed = http
        .post(format!("{endpoint}/heartbeat"))
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .json(&json!({"sequence":2}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(progressed["heartbeat_sequence"], 2);
    assert!(
        progressed["expires_at"].as_i64().unwrap() >= heartbeat["expires_at"].as_i64().unwrap()
    );
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
    let resumed = http
        .get(format!("{}/matchmaking/me", services.origin))
        .header("cookie", services.viewer.cookie())
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(resumed["state"], "waiting");
    assert_eq!(
        resumed["ticket"]["ticket_id"].as_str(),
        endpoints[0].rsplit('/').next()
    );
    for original in [
        http.delete(&endpoint),
        http.post(format!("{endpoint}/ready")),
        http.put(&endpoint).json(&json!({"pool":"ten_minute_30"})),
    ] {
        let recovered = original
            .header("cookie", services.viewer.cookie())
            .header("origin", &services.origin)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
        assert_eq!(recovered["state"], "cancelled");
    }
    let refusal_retry = http
        .put(&refused_endpoint)
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .json(&json!({"pool":"byoyomi_30"}))
        .send()
        .await
        .unwrap();
    assert_eq!(refusal_retry.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(refusal_retry.json::<Value>().await.unwrap(), refused);
    let terminal = http
        .post(format!("{endpoint}/heartbeat"))
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .json(&json!({"sequence":3}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(terminal["state"], "cancelled");
    assert!(terminal.get("expires_at").is_none());
    assert!(terminal.get("heartbeat_sequence").is_none());
    let unchanged = http
        .get(format!("{}/matchmaking/me", services.origin))
        .header("cookie", services.viewer.cookie())
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(unchanged, resumed);
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
    assert_eq!(current["state"], "matched");
    assert_eq!(current["ticket"]["ready"], false);
    assert!(current["ticket"]["ready_deadline"].is_i64());
    assert_eq!(
        services
            .client
            .clone()
            .get_snapshot(proto::GetSnapshotRequest {
                game_id: game.as_str().into(),
            })
            .await
            .unwrap_err()
            .code(),
        tonic::Code::NotFound
    );
    assert_eq!(current["ticket"]["game_id"], game.as_str());
    let tracked = http
        .post(format!("{}/heartbeat", endpoints[0]))
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .json(&json!({"sequence":1}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(tracked, current["ticket"]);
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
    assert_eq!(result["state"], "matched");
    let ready_endpoint = format!("{}/ready", endpoints[0]);
    let foreign = http
        .post(&ready_endpoint)
        .header("cookie", services.black.cookie())
        .header("origin", &services.origin)
        .send()
        .await
        .unwrap();
    assert_eq!(foreign.status(), reqwest::StatusCode::NOT_FOUND);
    assert_eq!(foreign.headers()["cache-control"], "no-store");
    let mut confirmation = None;
    for _ in 0..2 {
        let ready = http
            .post(&ready_endpoint)
            .header("cookie", services.viewer.cookie())
            .header("origin", &services.origin)
            .send()
            .await
            .unwrap();
        assert_eq!(ready.status(), reqwest::StatusCode::OK);
        assert_eq!(ready.headers()["cache-control"], "no-store");
        let ready: Value = ready.json().await.unwrap();
        assert_eq!(ready["state"], "matched");
        assert_eq!(ready["ready"], true);
        assert_eq!(ready["ready_deadline"], current["ticket"]["ready_deadline"]);
        if let Some(previous) = &confirmation {
            assert_eq!(&ready, previous);
        }
        confirmation = Some(ready);
    }
    assert_eq!(
        services
            .client
            .clone()
            .get_snapshot(proto::GetSnapshotRequest {
                game_id: game.as_str().into(),
            })
            .await
            .unwrap_err()
            .code(),
        tonic::Code::NotFound
    );
    let playing = http
        .post(format!("{}/ready", endpoints[1]))
        .header("cookie", opponent.cookie())
        .header("origin", &services.origin)
        .send()
        .await
        .unwrap();
    assert_eq!(playing.status(), reqwest::StatusCode::OK);
    let playing: Value = playing.json().await.unwrap();
    assert_eq!(playing["state"], "playing");
    assert_eq!(playing["game_id"], game.as_str());
    assert!(playing.get("ready").is_none());
    assert!(playing.get("ready_deadline").is_none());
    let snapshot = services
        .client
        .clone()
        .get_snapshot(proto::GetSnapshotRequest {
            game_id: game.as_str().into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(snapshot.seq, 0);
    let tracked = http
        .post(format!("{}/heartbeat", endpoints[0]))
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .json(&json!({"sequence":2}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(tracked["state"], "playing");
    assert_eq!(tracked["game_id"], playing["game_id"]);
    assert!(tracked.get("expires_at").is_none());
    assert!(tracked.get("heartbeat_sequence").is_none());
    assert!(tracked.get("ready_deadline").is_none());
    let playing_retry = http
        .post(&ready_endpoint)
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(playing_retry["state"], "playing");
    assert_eq!(playing_retry["game_id"], game.as_str());
    let cancel = http
        .delete(&endpoints[0])
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .send()
        .await
        .unwrap();
    assert_eq!(cancel.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(
        cancel.json::<Value>().await.unwrap()["code"],
        "already_matched"
    );
    services.stop().await;
}

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
        http.post(format!("{endpoint}/ready"))
            .header("cookie", services.viewer.cookie()),
    ] {
        let response = request.send().await.unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    for (cookie, origin, expected) in [
        (
            None,
            services.origin.as_str(),
            reqwest::StatusCode::UNAUTHORIZED,
        ),
        (
            Some(services.viewer.cookie()),
            "http://attacker.invalid",
            reqwest::StatusCode::FORBIDDEN,
        ),
        (
            Some(format!(
                "{}; {}",
                services.viewer.cookie(),
                services.viewer.cookie()
            )),
            services.origin.as_str(),
            reqwest::StatusCode::UNAUTHORIZED,
        ),
    ] {
        let request = http
            .post(format!("{endpoint}/ready"))
            .header("origin", origin);
        let request = match cookie {
            Some(cookie) => request.header("cookie", cookie),
            None => request,
        };
        let response = request.send().await.unwrap();
        assert_eq!(response.status(), expected);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    assert_eq!(
        http.get(format!("{}/matchmaking/me", services.origin))
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
        http.post(format!("{endpoint}/heartbeat"))
            .header("cookie", services.viewer.cookie())
            .header("origin", &services.origin)
            .json(&json!({"sequence":1}))
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

#[tokio::test]
async fn readiness_timeout_returns_a_durable_voided_pair_over_http() {
    use game_application::{matchmaking::MatchmakingRepository, rating::Pool};
    let services = Services::start().await;
    let opponent = login(&services.accounts, "timeout-opponent").await;
    let http = reqwest::Client::new();
    let mut endpoints = Vec::new();
    for browser in [&services.viewer, &opponent] {
        let endpoint = format!(
            "{}/matchmaking/tickets/{}",
            services.origin,
            uuid::Uuid::new_v4()
        );
        let joined = http
            .put(&endpoint)
            .header("cookie", browser.cookie())
            .header("origin", &services.origin)
            .json(&json!({"pool":"byoyomi_30"}))
            .send()
            .await
            .unwrap();
        assert_eq!(joined.status(), reqwest::StatusCode::CREATED);
        endpoints.push(endpoint);
    }
    let game = services.games.tick(Pool::Byoyomi30).await.unwrap().unwrap();
    let first_ready = http
        .post(format!("{}/ready", endpoints[0]))
        .header("cookie", services.viewer.cookie())
        .header("origin", &services.origin)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(first_ready["ready"], true);
    let raw = Database::connect(services.game_database.url())
        .await
        .unwrap();
    raw.execute_unprepared("UPDATE matchmaking_tickets SET ready_deadline=0 WHERE state='matched'")
        .await
        .unwrap();
    for (browser, endpoint) in [
        (&services.viewer, &endpoints[0]),
        (&opponent, &endpoints[1]),
    ] {
        let heartbeat = http
            .post(format!("{endpoint}/heartbeat"))
            .header("cookie", browser.cookie())
            .header("origin", &services.origin)
            .json(&json!({"sequence":1}))
            .send()
            .await
            .unwrap();
        assert_eq!(heartbeat.status(), reqwest::StatusCode::OK);
        let terminal = heartbeat.json::<Value>().await.unwrap();
        assert_eq!(terminal["state"], "voided");
        assert!(terminal.get("expires_at").is_none());
        assert!(terminal.get("heartbeat_sequence").is_none());
        let ready = http
            .post(format!("{endpoint}/ready"))
            .header("cookie", browser.cookie())
            .header("origin", &services.origin)
            .send()
            .await
            .unwrap();
        assert_eq!(ready.status(), reqwest::StatusCode::OK);
        assert_eq!(ready.headers()["cache-control"], "no-store");
        let voided: Value = ready.json().await.unwrap();
        assert_eq!(voided, terminal);
        assert_eq!(voided["state"], "voided");
        assert_eq!(voided["game_id"], game.as_str());
        assert!(voided["side"].is_string());
        assert!(voided.get("ready").is_none());
        assert!(voided.get("ready_deadline").is_none());
        for request in [
            http.post(format!("{endpoint}/ready")),
            http.delete(endpoint),
        ] {
            let retry = request
                .header("cookie", browser.cookie())
                .header("origin", &services.origin)
                .send()
                .await
                .unwrap();
            assert_eq!(retry.status(), reqwest::StatusCode::OK);
            assert_eq!(retry.json::<Value>().await.unwrap(), voided);
        }
    }
    assert_eq!(
        services
            .client
            .clone()
            .get_snapshot(proto::GetSnapshotRequest {
                game_id: game.as_str().into(),
            })
            .await
            .unwrap_err()
            .code(),
        tonic::Code::NotFound
    );
    raw.close().await.unwrap();
    services.stop().await;
}
