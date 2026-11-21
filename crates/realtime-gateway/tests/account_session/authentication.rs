use super::*;

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
