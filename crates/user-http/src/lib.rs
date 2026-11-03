//! Browser account routes. OAuth credentials stay on the server.
mod config;
mod google;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
pub use config::{BrowserConfig, ConfigError, ServerConfig};
pub use google::{Authorization, GOOGLE_ISSUER, GoogleProvider, IdentityProvider};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use user_application::{AccountRepository, DisplayName, Error, LoginAttempt, NewSession, Profile};

#[derive(Clone)]
struct App {
    store: Arc<dyn AccountRepository>,
    provider: Arc<dyn IdentityProvider>,
    browser: BrowserConfig,
}
pub fn router(
    store: Arc<dyn AccountRepository>,
    provider: Arc<dyn IdentityProvider>,
    browser: BrowserConfig,
) -> Router {
    Router::new()
        .route("/auth/google/start", get(start))
        .route("/auth/google/callback", get(callback))
        .route("/me", get(me).patch(rename))
        .route("/auth/logout", post(logout))
        .layer(middleware::map_response(private_response))
        .with_state(App {
            store,
            provider,
            browser,
        })
}
async fn private_response(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs() as i64
}
fn random() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
fn hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Result<&'a str, HttpError> {
    let mut found = None;
    for line in headers.get_all(header::COOKIE) {
        for item in line
            .to_str()
            .map_err(|_| HttpError(Error::Unauthorized))?
            .split(';')
        {
            if let Some((key, value)) = item.trim().split_once('=')
                && key == name
            {
                if found.is_some()
                    || value.len() != 64
                    || !value.bytes().all(|c| c.is_ascii_hexdigit())
                {
                    return Err(HttpError(Error::Unauthorized));
                }
                found = Some(value);
            }
        }
    }
    found.ok_or(HttpError(Error::Unauthorized))
}
fn set_cookie(
    response: &mut Response,
    config: &BrowserConfig,
    name: &str,
    value: &str,
    seconds: i64,
) {
    let secure = if config.secure { "; Secure" } else { "" };
    let value =
        format!("{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={seconds}{secure}");
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&value).expect("generated cookie contains ASCII"),
    );
}
fn same_origin(headers: &HeaderMap, config: &BrowserConfig) -> Result<(), HttpError> {
    if headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) != Some(config.origin.as_str()) {
        return Err(HttpError(Error::Unauthorized));
    }
    if headers
        .get("sec-fetch-site")
        .is_some_and(|v| v != "same-origin")
    {
        return Err(HttpError(Error::Unauthorized));
    }
    Ok(())
}
struct HttpError(Error);
impl From<Error> for HttpError {
    fn from(e: Error) -> Self {
        Self(e)
    }
}
impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let code = match self.0 {
            Error::Unauthorized => StatusCode::UNAUTHORIZED,
            Error::InvalidInput => StatusCode::BAD_REQUEST,
            Error::Unavailable | Error::Corrupt => StatusCode::SERVICE_UNAVAILABLE,
        };
        (code, Json(serde_json_error(self.0))).into_response()
    }
}
#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}
fn serde_json_error(e: Error) -> ErrorBody {
    ErrorBody {
        error: match e {
            Error::Unauthorized => "authentication_required",
            Error::InvalidInput => "invalid_input",
            _ => "service_unavailable",
        },
    }
}
async fn start(State(app): State<App>) -> Result<Response, HttpError> {
    let a = app.provider.authorize();
    let browser = random();
    app.store
        .save_attempt(
            LoginAttempt {
                state_hash: hash(&a.state),
                browser_hash: hash(&browser),
                nonce: a.nonce,
                pkce_verifier: a.pkce_verifier,
                expires_at: now() + 600,
            },
            now(),
        )
        .await?;
    let mut response = Redirect::to(&a.url).into_response();
    set_cookie(
        &mut response,
        &app.browser,
        app.browser.login_cookie(),
        &browser,
        600,
    );
    Ok(response)
}
#[derive(Deserialize)]
struct Callback {
    state: String,
    code: Option<String>,
    error: Option<String>,
}
async fn callback(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<Callback>,
) -> Result<Response, HttpError> {
    if query.state.len() > 1024 {
        return Err(Error::Unauthorized.into());
    }
    let browser = cookie(&headers, app.browser.login_cookie())?;
    let attempt = app
        .store
        .consume_attempt(&hash(&query.state), &hash(browser), now())
        .await?;
    if query.error.is_some() {
        return Err(Error::Unauthorized.into());
    }
    let code = query
        .code
        .filter(|c| !c.is_empty() && c.len() <= 4096)
        .ok_or(HttpError(Error::Unauthorized))?;
    let identity = app.provider.exchange(code, attempt).await?;
    let token = random();
    app.store
        .login(
            identity,
            NewSession {
                token_hash: hash(&token),
                expires_at: now() + app.browser.session_seconds,
            },
            now(),
        )
        .await?;
    if let Ok(old) = cookie(&headers, app.browser.session_cookie()) {
        app.store.logout(&hash(old)).await?;
    }
    let mut response = Redirect::to("/me").into_response();
    set_cookie(
        &mut response,
        &app.browser,
        app.browser.session_cookie(),
        &token,
        app.browser.session_seconds,
    );
    set_cookie(
        &mut response,
        &app.browser,
        app.browser.login_cookie(),
        "",
        0,
    );
    Ok(response)
}
#[derive(Serialize)]
struct ProfileBody {
    id: String,
    display_name: String,
    created_at: i64,
}
impl From<Profile> for ProfileBody {
    fn from(p: Profile) -> Self {
        Self {
            id: p.id.as_str().into(),
            display_name: p.display_name.as_str().into(),
            created_at: p.created_at,
        }
    }
}
async fn me(State(app): State<App>, headers: HeaderMap) -> Result<Json<ProfileBody>, HttpError> {
    Ok(Json(
        app.store
            .authenticate(
                &hash(cookie(&headers, app.browser.session_cookie())?),
                now(),
            )
            .await?
            .into(),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rename {
    display_name: String,
}
async fn rename(
    State(app): State<App>,
    headers: HeaderMap,
    Json(body): Json<Rename>,
) -> Result<Json<ProfileBody>, HttpError> {
    same_origin(&headers, &app.browser)?;
    let name = DisplayName::new(body.display_name)?;
    Ok(Json(
        app.store
            .rename(
                &hash(cookie(&headers, app.browser.session_cookie())?),
                name,
                now(),
            )
            .await?
            .into(),
    ))
}
async fn logout(State(app): State<App>, headers: HeaderMap) -> Result<Response, HttpError> {
    same_origin(&headers, &app.browser)?;
    if let Ok(token) = cookie(&headers, app.browser.session_cookie()) {
        app.store.logout(&hash(token)).await?;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    set_cookie(
        &mut response,
        &app.browser,
        app.browser.session_cookie(),
        "",
        0,
    );
    Ok(response)
}
#[cfg(test)]
mod tests;
