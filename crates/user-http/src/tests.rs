use super::*;
use async_trait::async_trait;
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;
use user_application::VerifiedIdentity;
use user_persistence::SqliteAccounts;

#[derive(Default)]
struct Provider {
    exchanges: AtomicUsize,
}
#[async_trait]
impl IdentityProvider for Provider {
    fn authorize(&self) -> Authorization {
        let state = random();
        Authorization {
            url: format!("https://accounts.google.com/auth?state={state}"),
            state,
            nonce: "expected-nonce".into(),
            pkce_verifier: "expected-verifier".into(),
        }
    }
    async fn exchange(
        &self,
        code: String,
        attempt: LoginAttempt,
    ) -> Result<VerifiedIdentity, Error> {
        self.exchanges.fetch_add(1, Ordering::SeqCst);
        if code != "valid"
            || attempt.nonce != "expected-nonce"
            || attempt.pkce_verifier != "expected-verifier"
        {
            return Err(Error::Unauthorized);
        }
        Ok(VerifiedIdentity {
            issuer: GOOGLE_ISSUER.into(),
            subject: "google-subject".into(),
            initial_name: DisplayName::new("Google Player".into()).unwrap(),
        })
    }
}
async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    origin: Option<&str>,
    body: &str,
) -> Response {
    let mut r = Request::builder().method(method).uri(uri);
    if let Some(c) = cookie {
        r = r.header(header::COOKIE, c);
    }
    if let Some(o) = origin {
        r = r.header(header::ORIGIN, o);
    }
    if !body.is_empty() {
        r = r.header(header::CONTENT_TYPE, "application/json");
    }
    app.clone()
        .oneshot(r.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap()
}
fn response_cookie(response: &Response, name: &str) -> String {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with(&format!("{name}=")))
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .into()
}
async fn begin(app: &Router) -> (String, String) {
    let r = call(app, "GET", "/auth/google/start", None, None, "").await;
    assert_eq!(r.status(), StatusCode::SEE_OTHER);
    assert_eq!(r.headers()[header::CACHE_CONTROL], "no-store");
    let state = url::Url::parse(r.headers()[header::LOCATION].to_str().unwrap())
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned();
    (state, response_cookie(&r, "user_login"))
}
async fn login(app: &Router) -> String {
    let (state, browser) = begin(app).await;
    let r = call(
        app,
        "GET",
        &format!("/auth/google/callback?state={state}&code=valid"),
        Some(&browser),
        None,
        "",
    )
    .await;
    assert_eq!(r.status(), StatusCode::SEE_OTHER);
    response_cookie(&r, "user_session")
}
async fn json(r: Response) -> serde_json::Value {
    serde_json::from_slice(&r.into_body().collect().await.unwrap().to_bytes()).unwrap()
}
fn app(store: Arc<SqliteAccounts>, provider: Arc<Provider>) -> Router {
    router(
        store,
        provider,
        BrowserConfig::new("http://localhost:8080", true).unwrap(),
    )
}

#[tokio::test]
async fn registration_profile_update_csrf_logout_and_restart() {
    let temp = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        temp.path().join("users.db").display()
    );
    let store = Arc::new(SqliteAccounts::connect(&url).await.unwrap());
    let provider = Arc::new(Provider::default());
    let first = app(store.clone(), provider.clone());
    assert_eq!(
        call(&first, "GET", "/me", None, None, "").await.status(),
        StatusCode::UNAUTHORIZED
    );
    let session = login(&first).await;
    let profile = json(call(&first, "GET", "/me", Some(&session), None, "").await).await;
    assert_eq!(profile["display_name"], "Google Player");
    assert!(uuid::Uuid::parse_str(profile["id"].as_str().unwrap()).is_ok());
    assert_eq!(
        call(
            &first,
            "PATCH",
            "/me",
            Some(&session),
            None,
            r#"{"display_name":"new"}"#
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &first,
            "PATCH",
            "/me",
            Some(&session),
            Some("https://attacker.invalid"),
            r#"{"display_name":"new"}"#
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &first,
            "PATCH",
            "/me",
            Some(&session),
            Some("http://localhost:8080"),
            r#"{"display_name":"   "}"#
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let renamed = json(
        call(
            &first,
            "PATCH",
            "/me",
            Some(&session),
            Some("http://localhost:8080"),
            r#"{"display_name":"New Name"}"#,
        )
        .await,
    )
    .await;
    assert_eq!(renamed["display_name"], "New Name");
    drop(first);
    store.close().await.unwrap();
    let restarted_store = Arc::new(SqliteAccounts::connect(&url).await.unwrap());
    let restarted = app(restarted_store.clone(), provider);
    assert_eq!(
        json(call(&restarted, "GET", "/me", Some(&session), None, "").await).await,
        renamed
    );
    let second_session = login(&restarted).await;
    assert_eq!(
        json(call(&restarted, "GET", "/me", Some(&second_session), None, "").await).await,
        renamed
    );
    assert_eq!(
        call(&restarted, "POST", "/auth/logout", Some(&session), None, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &restarted,
            "POST",
            "/auth/logout",
            Some(&session),
            Some("http://localhost:8080"),
            ""
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&restarted, "GET", "/me", Some(&session), None, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&restarted, "GET", "/me", Some(&second_session), None, "")
            .await
            .status(),
        StatusCode::OK
    );
    restarted_store.close().await.unwrap();
    let final_store = Arc::new(SqliteAccounts::connect(&url).await.unwrap());
    assert_eq!(
        call(
            &app(final_store, Arc::new(Provider::default())),
            "GET",
            "/me",
            Some(&session),
            None,
            ""
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn login_is_browser_bound_and_consumed_before_any_exchange() {
    let store = Arc::new(SqliteAccounts::connect("sqlite::memory:").await.unwrap());
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
async fn tampered_and_duplicate_session_cookies_never_authenticate() {
    let store = Arc::new(SqliteAccounts::connect("sqlite::memory:").await.unwrap());
    let app = app(store, Arc::new(Provider::default()));
    let session = login(&app).await;
    let mut changed = session.clone();
    changed.pop();
    changed.push(if session.ends_with('a') { 'b' } else { 'a' });
    assert_eq!(
        call(&app, "GET", "/me", Some(&changed), None, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "GET",
            "/me",
            Some(&format!("{session}; {session}")),
            None,
            ""
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn production_cookies_have_host_prefix_and_secure_attributes() {
    let store = Arc::new(SqliteAccounts::connect("sqlite::memory:").await.unwrap());
    let app = router(
        store,
        Arc::new(Provider::default()),
        BrowserConfig::new("https://users.example.com", false).unwrap(),
    );
    let r = call(&app, "GET", "/auth/google/start", None, None, "").await;
    let c = r.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(c.starts_with("__Host-user_login="));
    for required in [
        "Secure",
        "HttpOnly",
        "SameSite=Lax",
        "Path=/",
        "Max-Age=600",
    ] {
        assert!(c.contains(required));
    }
    assert!(!c.contains("Domain="));
}

#[test]
fn configuration_fails_closed() {
    for bad in [
        "http://users.example.com",
        "http://localhost:8080",
        "https://u:p@example.com",
        "https://example.com/path",
        "https://example.com?x=y",
        "https://example.com/#secret",
        "file:///tmp/file",
    ] {
        assert!(BrowserConfig::new(bad, false).is_err(), "{bad}");
    }
    for bad in [
        "http://evil.invalid",
        "http://localhost.evil.invalid",
        "http://192.168.1.2",
    ] {
        assert!(BrowserConfig::new(bad, true).is_err(), "{bad}");
    }
    assert!(BrowserConfig::new("http://localhost:8080", true).is_ok());
    assert!(BrowserConfig::new("http://[::1]:8080", true).is_ok());
    assert_eq!(
        BrowserConfig::new("https://users.example.com", false)
            .unwrap()
            .callback_url(),
        "https://users.example.com/auth/google/callback"
    );
}

#[tokio::test]
async fn browser_flow_over_real_tcp_http_routes() {
    let store = Arc::new(SqliteAccounts::connect("sqlite::memory:").await.unwrap());
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
