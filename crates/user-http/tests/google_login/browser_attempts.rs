use super::*;

#[tokio::test]
async fn login_is_browser_bound_and_consumed_before_any_exchange() {
    let database = pg_test_support::TestDatabase::new().await;
    let store = Arc::new(PostgresAccounts::connect(database.url()).await.unwrap());
    let provider = Arc::new(Provider::default());
    let app = app(store, provider.clone());
    let (state, browser) = begin(&app).await;
    let callback = format!("/auth/google/callback?state={state}&code=valid");
    assert_eq!(
        call(&app, "GET", &callback, None, None, "").await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "GET",
            &callback,
            Some(&format!("user_login={}", random())),
            None,
            ""
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(provider.exchanges.load(Ordering::SeqCst), 0);
    assert_eq!(
        call(&app, "GET", &callback, Some(&browser), None, "")
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        call(&app, "GET", &callback, Some(&browser), None, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(provider.exchanges.load(Ordering::SeqCst), 1);
    let (state, browser) = begin(&app).await;
    let bad = format!("/auth/google/callback?state={state}&code=bad");
    assert_eq!(
        call(&app, "GET", &bad, Some(&browser), None, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let retry = format!("/auth/google/callback?state={state}&code=valid");
    assert_eq!(
        call(&app, "GET", &retry, Some(&browser), None, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(provider.exchanges.load(Ordering::SeqCst), 2);
}

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

#[tokio::test]
async fn persisted_browser_credentials_are_hashes_not_cookie_values() {
    use sea_orm::{
        ConnectionTrait, Database,
        sea_query::{Alias, Query},
    };
    let database = pg_test_support::TestDatabase::new().await;
    let store = Arc::new(PostgresAccounts::connect(database.url()).await.unwrap());
    let app = app(store, Arc::new(Provider::default()));
    let raw = Database::connect(database.url()).await.unwrap();
    let (state, browser) = begin(&app).await;
    let browser_value = browser.split_once('=').unwrap().1;
    let attempt = raw
        .query_one(
            Query::select()
                .columns([Alias::new("state_hash"), Alias::new("browser_hash")])
                .from(Alias::new("login_attempts")),
        )
        .await
        .unwrap()
        .unwrap();
    let saved_state: String = attempt.try_get("", "state_hash").unwrap();
    let saved_browser: String = attempt.try_get("", "browser_hash").unwrap();
    assert_ne!(saved_state, state);
    assert_ne!(saved_browser, browser_value);
    assert_eq!(saved_state, hash(&state));
    assert_eq!(saved_browser, hash(browser_value));
    let response = call(
        &app,
        "GET",
        &format!("/auth/google/callback?state={state}&code=valid"),
        Some(&browser),
        None,
        "",
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = response_cookie(&response, "user_session");
    let token = cookie.split_once('=').unwrap().1;
    let session = raw
        .query_one(
            Query::select()
                .column(Alias::new("token_hash"))
                .from(Alias::new("sessions")),
        )
        .await
        .unwrap()
        .unwrap();
    let saved: String = session.try_get("", "token_hash").unwrap();
    assert_ne!(saved, token);
    assert_eq!(saved, hash(token));
}
