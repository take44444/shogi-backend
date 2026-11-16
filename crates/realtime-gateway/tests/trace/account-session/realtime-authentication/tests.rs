use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実 user HTTP・PostgreSQL・gRPC・Valkey と WebSocket ハンドシェイク。未知 token の本人確認を含む。",
        reason: "存在しないが形式の正しい token は実 user HTTP に問い合わせて 401 となり、欠落・不正・重複 Cookie もハンドシェイクで拒否する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "browser-headers", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "Originの欠落・不正なオリジン・重複、クロスサイトのメタデータ、プロトコルの欠落・不一致を403で拒否する。" }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "session-auth", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "セッションCookieの欠落・重複・不正・未知の値を401で拒否し、アップグレードしない。" }],
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

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実サービス群と WebSocket。HTTP logout と SQL による期限切れを起こし、次のコマンド・通知の拒否を確認する。",
        reason: "有効な user HTTP セッションで WebSocket 接続する。",
        relationship: crate::trace_support::Relationship::Exercises,
        also: &[crate::trace_support::Evidence { feature: "account-session", contract: "realtime-recheck", scope: "実サービス群と WebSocket。HTTP logout と SQL による期限切れを起こし、次のコマンド・通知の拒否を確認する。", relationship: crate::trace_support::Relationship::Asserts, reason: "実 HTTP logout の後のコマンドで接続が閉じ game seq が変わらず、DB で期限切れにした viewer は次の通知で閉じ、双方の再接続も 401 となることを確認する。" }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "session-auth", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。 実際のアカウントのログアウトと、セッションの強制的な期限切れを使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "コマンド前の実際のログアウトと通知前の期限切れでソケットを閉じ、その後のアップグレードは401となる。" }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "live-checks", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。 実際のアカウントのログアウトと、セッションの強制的な期限切れを使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "着手の送信前にログアウトすると接続が閉じ、永続化された連番は0のままとなる。" }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "outgoing", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。 実際のアカウントのログアウトと、セッションの強制的な期限切れを使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "セッションが期限切れになった閲覧者への接続は、配信された投了イベントを届ける前に閉じる。" }],
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
