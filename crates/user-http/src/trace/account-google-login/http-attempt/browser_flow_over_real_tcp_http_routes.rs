#[tokio::test]
async fn browser_flow_over_real_tcp_http_routes() {
    let database = pg_test_support::TestDatabase::new().await;
    let store = Arc::new(PostgresAccounts::connect(database.url()).await.unwrap());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://localhost:{}", listener.local_addr().unwrap().port());
    let app = router(
        store,
        Arc::new(Provider::default()),
        BrowserConfig::new(&origin, true).unwrap(),
    );
    let serving = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let http = openidconnect::reqwest::Client::builder()
        .redirect(openidconnect::reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let start = http
        .get(format!("{origin}/auth/google/start"))
        .send()
        .await
        .unwrap();
    assert_eq!(start.status(), StatusCode::SEE_OTHER);
    let browser = start.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let state = url::Url::parse(start.headers()[header::LOCATION].to_str().unwrap())
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    let callback = http
        .get(format!(
            "{origin}/auth/google/callback?state={state}&code=valid"
        ))
        .header(header::COOKIE, browser)
        .send()
        .await
        .unwrap();
    assert_eq!(callback.status(), StatusCode::SEE_OTHER);
    let session = callback
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .find_map(|c| {
            c.to_str()
                .ok()
                .filter(|c| c.starts_with("user_session="))
                .map(|c| c.split(';').next().unwrap().to_owned())
        })
        .unwrap();
    let me = http
        .get(format!("{origin}/me"))
        .header(header::COOKIE, &session)
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::OK);
    let profile: serde_json::Value = serde_json::from_slice(&me.bytes().await.unwrap()).unwrap();
    assert_eq!(profile["display_name"], "Google Player");
    let out = http
        .post(format!("{origin}/auth/logout"))
        .header(header::COOKIE, &session)
        .header(header::ORIGIN, &origin)
        .send()
        .await
        .unwrap();
    assert_eq!(out.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        http.get(format!("{origin}/me"))
            .header(header::COOKIE, &session)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    serving.abort();
}
