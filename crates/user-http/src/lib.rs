//! Browser account routes. OAuth credentials stay on the server.
mod config;
mod google;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware,
    response::{IntoResponse, Redirect, Response},
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
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

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
    api_router()
        .split_for_parts()
        .0
        .layer(middleware::map_response(private_response))
        .with_state(App {
            store,
            provider,
            browser,
        })
}
/// Construct the document without configuration, persistence, or provider calls.
pub fn openapi() -> utoipa::openapi::OpenApi {
    api_router().into_openapi()
}
#[derive(utoipa::OpenApi)]
#[openapi(modifiers(&CookieSecurity))]
struct Api;
struct CookieSecurity;
impl utoipa::Modify for CookieSecurity {
    fn modify(&self, api: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
        api.info = utoipa::openapi::Info::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        let components = api.components.get_or_insert_with(Default::default);
        for (scheme, name, description) in [
            (
                "session",
                "__Host-user_session",
                "HTTPS production session cookie",
            ),
            (
                "development_session",
                "user_session",
                "Explicit loopback HTTP development session cookie",
            ),
            (
                "login",
                "__Host-user_login",
                "HTTPS production browser-bound login attempt cookie",
            ),
            (
                "development_login",
                "user_login",
                "Explicit loopback HTTP development login attempt cookie",
            ),
        ] {
            components.add_security_scheme(
                scheme,
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                    name,
                    description,
                ))),
            );
        }
        api.info.description = Some("Opaque browser cookies. HTTPS uses host-only __Host- cookies with Secure, HttpOnly, SameSite=Lax and Path=/. Explicit loopback HTTP development mode uses the alternative cookie names without Secure. All responses have Cache-Control: no-store, Referrer-Policy: no-referrer and X-Content-Type-Options: nosniff.".into());
    }
}
fn api_router() -> OpenApiRouter<App> {
    use utoipa::OpenApi;
    OpenApiRouter::with_openapi(Api::openapi())
        .routes(routes!(start))
        .routes(routes!(callback))
        .routes(routes!(me))
        .routes(routes!(rename))
        .routes(routes!(logout))
        .routes(routes!(openapi_json))
}
#[utoipa::path(get, path = "/openapi.json", responses(
    (status = 200, description = "OpenAPI document", body = Object, content_type = "application/json")
))]
async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi())
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
#[derive(Serialize, ToSchema)]
struct ErrorBody {
    error: ErrorCode,
}
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
enum ErrorCode {
    AuthenticationRequired,
    InvalidInput,
    ServiceUnavailable,
}
fn serde_json_error(e: Error) -> ErrorBody {
    ErrorBody {
        error: match e {
            Error::Unauthorized => ErrorCode::AuthenticationRequired,
            Error::InvalidInput => ErrorCode::InvalidInput,
            _ => ErrorCode::ServiceUnavailable,
        },
    }
}
/// Create a ten-minute browser-bound login attempt. No session is required.
#[utoipa::path(get, path = "/auth/google/start", responses(
    (status = 303, description = "Redirect to Google", headers(
        ("Location" = String, description = "Google authorization URL"),
        ("Set-Cookie" = String, description = "Login cookie with Max-Age=600")
    )),
    (status = 400, description = "Invalid input", body = ErrorBody),
    (status = 401, description = "Authentication required or browser check failed", body = ErrorBody),
    (status = 503, description = "Service unavailable", body = ErrorBody)
))]
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
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct Callback {
    /// Required single-use state, at most 1024 bytes, bound to the login cookie.
    state: String,
    /// Required for success, nonempty and at most 4096 bytes.
    code: Option<String>,
    /// Provider error. Presence rejects authentication.
    error: Option<String>,
}
/// Consume the browser-bound, single-use login attempt. Successful login rotates the session.
#[utoipa::path(get, path = "/auth/google/callback", params(Callback),
    security(("login" = []), ("development_login" = [])), responses(
    (status = 303, description = "Session created", headers(
        ("Location" = String, description = "/me"),
        ("Set-Cookie" = String, description = "Two Set-Cookie headers set the session and clear the login cookie")
    )),
    (status = 400, description = "Malformed or missing callback query (text), or invalid input (JSON)", content(
        (String = "text/plain"), (ErrorBody = "application/json")
    )),
    (status = 401, description = "Authentication required or browser check failed", body = ErrorBody),
    (status = 503, description = "Service unavailable", body = ErrorBody)
))]
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
#[derive(Serialize, ToSchema)]
struct ProfileBody {
    id: String,
    display_name: String,
    /// UTC seconds since the Unix epoch.
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
#[utoipa::path(get, path = "/me", security(("session" = []), ("development_session" = [])), responses(
    (status = 200, description = "Authenticated profile", body = ProfileBody),
    (status = 400, description = "Invalid input", body = ErrorBody),
    (status = 401, description = "Authentication required or browser check failed", body = ErrorBody),
    (status = 503, description = "Service unavailable", body = ErrorBody)
))]
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
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct Rename {
    /// Trimmed before validation. Must then contain 1..80 Unicode characters and no controls.
    display_name: String,
}
/// Require matching Origin and, when supplied, Sec-Fetch-Site: same-origin.
#[utoipa::path(patch, path = "/me", request_body = Rename, params(
    ("Origin" = String, Header, description = "Required exact match to configured public origin"),
    ("Sec-Fetch-Site" = Option<String>, Header, description = "If supplied, must be same-origin")
),
    security(("session" = []), ("development_session" = [])), responses(
    (status = 200, description = "Updated profile", body = ProfileBody),
    (status = 400, description = "Malformed JSON (text) or invalid trimmed display name (JSON)", content(
        (String = "text/plain"), (ErrorBody = "application/json")
    )),
    (status = 413, description = "Request body too large", body = String, content_type = "text/plain"),
    (status = 415, description = "Missing or unsupported JSON content type", body = String, content_type = "text/plain"),
    (status = 422, description = "Invalid JSON structure, including unknown fields", body = String, content_type = "text/plain"),
    (status = 401, description = "Authentication required or browser check failed", body = ErrorBody),
    (status = 503, description = "Service unavailable", body = ErrorBody)
))]
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
/// Require matching Origin and, when supplied, Sec-Fetch-Site: same-origin.
/// A missing or invalid session cookie is allowed. A valid session is revoked.
#[utoipa::path(post, path = "/auth/logout", params(
    ("Origin" = String, Header, description = "Required exact match to configured public origin"),
    ("Sec-Fetch-Site" = Option<String>, Header, description = "If supplied, must be same-origin")
), responses(
    (status = 204, description = "Session cookie cleared", headers(
        ("Set-Cookie" = String, description = "Session cookie with Max-Age=0")
    )),
    (status = 400, description = "Invalid input", body = ErrorBody),
    (status = 401, description = "Authentication required or browser check failed", body = ErrorBody),
    (status = 503, description = "Service unavailable", body = ErrorBody)
))]
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

#[cfg(test)]
mod openapi_tests;
