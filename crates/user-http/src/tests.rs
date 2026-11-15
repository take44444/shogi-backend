use super::*;
use async_trait::async_trait;
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;
use user_application::VerifiedIdentity;
use user_persistence::PostgresAccounts;

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
fn app(store: Arc<PostgresAccounts>, provider: Arc<Provider>) -> Router {
    router(
        store,
        provider,
        BrowserConfig::new("http://localhost:8080", true).unwrap(),
    )
}

include!(
    "trace/account-profile/existing-identity/registration_profile_update_csrf_logout_and_restart.rs"
);

include!(
    "trace/account-google-login/http-attempt/login_is_browser_bound_and_consumed_before_any_exchange.rs"
);

include!(
    "trace/account-session/session-cookie-input/tampered_and_duplicate_session_cookies_never_authenticate.rs"
);

include!(
    "trace/account-session/cookie-issuance/production_cookies_have_host_prefix_and_secure_attributes.rs"
);

include!("trace/startup-schema-isolation/contract-browser-config/configuration_fails_closed.rs");

include!("trace/account-google-login/http-attempt/browser_flow_over_real_tcp_http_routes.rs");

include!(
    "trace/account-google-login/http-attempt/persisted_browser_credentials_are_hashes_not_cookie_values.rs"
);
