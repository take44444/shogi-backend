use super::*;

pub(super) fn random() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
pub(super) fn hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
pub(super) use async_trait::async_trait;
pub(super) use axum::{body::Body, http::Request};
pub(super) use http_body_util::BodyExt;
pub(super) use std::sync::atomic::{AtomicUsize, Ordering};
pub(super) use tower::ServiceExt;
pub(super) use user_application::VerifiedIdentity;
pub(super) use user_persistence::PostgresAccounts;

#[derive(Default)]
pub(super) struct Provider {
    pub(super) exchanges: AtomicUsize,
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
pub(super) async fn call(
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
pub(super) fn response_cookie(response: &Response, name: &str) -> String {
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
pub(super) async fn begin(app: &Router) -> (String, String) {
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
pub(super) async fn login(app: &Router) -> String {
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
pub(super) async fn json(r: Response) -> serde_json::Value {
    serde_json::from_slice(&r.into_body().collect().await.unwrap().to_bytes()).unwrap()
}
pub(super) fn app(store: Arc<PostgresAccounts>, provider: Arc<Provider>) -> Router {
    router(
        store,
        provider,
        BrowserConfig::new("http://localhost:8080", true).unwrap(),
    )
}
