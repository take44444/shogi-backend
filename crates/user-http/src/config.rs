use std::{env, net::SocketAddr};
use url::Url;

#[derive(Clone)]
pub struct BrowserConfig {
    pub(crate) origin: String,
    pub(crate) secure: bool,
    pub(crate) session_seconds: i64,
}
#[derive(Debug, thiserror::Error)]
#[error("invalid user service configuration: {0}")]
pub struct ConfigError(pub &'static str);
impl BrowserConfig {
    pub fn new(origin: &str, allow_loopback_http: bool) -> Result<Self, ConfigError> {
        let url =
            Url::parse(origin).map_err(|_| ConfigError("USER_PUBLIC_ORIGIN must be a URL"))?;
        let secure = url.scheme() == "https";
        let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if (!secure && !(allow_loopback_http && url.scheme() == "http" && loopback))
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(ConfigError(
                "origin must be HTTPS without path, credentials, query or fragment; HTTP requires explicit loopback development mode",
            ));
        }
        Ok(Self {
            origin: url.origin().ascii_serialization(),
            secure,
            session_seconds: 86400,
        })
    }
    pub fn callback_url(&self) -> String {
        format!("{}/auth/google/callback", self.origin)
    }
    pub(crate) fn session_cookie(&self) -> &'static str {
        if self.secure {
            "__Host-user_session"
        } else {
            "user_session"
        }
    }
    pub(crate) fn login_cookie(&self) -> &'static str {
        if self.secure {
            "__Host-user_login"
        } else {
            "user_login"
        }
    }
}

pub struct ServerConfig {
    pub browser: BrowserConfig,
    pub bind: SocketAddr,
    pub database_url: String,
    pub google_client_id: String,
    pub google_client_secret: String,
}
impl ServerConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let required = |name| {
            env::var(name)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .ok_or(ConfigError(name))
        };
        let dev = match env::var("USER_ALLOW_LOOPBACK_HTTP").as_deref() {
            Ok("true") => true,
            Ok("false") | Err(_) => false,
            _ => {
                return Err(ConfigError(
                    "USER_ALLOW_LOOPBACK_HTTP must be true or false",
                ));
            }
        };
        let mut browser = BrowserConfig::new(&required("USER_PUBLIC_ORIGIN")?, dev)?;
        if let Ok(ttl) = env::var("USER_SESSION_SECONDS") {
            browser.session_seconds = ttl
                .parse()
                .ok()
                .filter(|v| (60..=2592000).contains(v))
                .ok_or(ConfigError("USER_SESSION_SECONDS must be 60..=2592000"))?;
        }
        let bind: SocketAddr = env::var("USER_BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8080".into())
            .parse()
            .map_err(|_| ConfigError("USER_BIND_ADDR must be a socket address"))?;
        let database_url = required("USER_DATABASE_URL")?;
        if !database_url.starts_with("sqlite:") {
            return Err(ConfigError("USER_DATABASE_URL must be SQLite"));
        }
        Ok(Self {
            browser,
            bind,
            database_url,
            google_client_id: required("GOOGLE_CLIENT_ID")?,
            google_client_secret: required("GOOGLE_CLIENT_SECRET")?,
        })
    }
}
