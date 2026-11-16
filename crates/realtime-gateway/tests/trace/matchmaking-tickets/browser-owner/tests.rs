use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。",
        reason: "未認証での現在状態の取得は401、別の所有者は404、Origin・Hostの不一致は403となる。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "reads", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "ブラウザーが、チケットの識別情報を使って現在の対局中の対局と保存済みの拒否結果を取得する。" }, crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "heartbeat", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "HTTPでは連番0を拒否し、連番1の重複リクエストには同じチケットを返す。" }, crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "rpc-boundary", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Exercises, reason: "ブラウザーからの呼び出しで5種類すべてのRPCを通し、現在状態、保存済み状態、ハートビート、キャンセルについて検証した結果を返す。" }, crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "browser-response", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "新規参加はno-store付きの201、再試行は同じJSONの200、プールの競合は409となる。対局中のキャンセルは、game_id付きの409 already_matchedとなる。" }],
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

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。",
        reason: "変更操作のOrigin欠落、Cookieの重複、未知のプール、偽のuser_idを拒否する。拒否した参加要求ではチケットを残さない。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "browser-response", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "Originを欠く変更操作への応答にもno-storeを付ける。" }],
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
