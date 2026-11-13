use axum::http::{HeaderMap, header};
use futures_util::StreamExt;
use game_application::UserId;
use reqwest::{Client, Url};
use serde::Deserialize;
use std::time::Duration;

#[derive(Clone)]
pub(crate) struct Session {
    pub(crate) cookie: String,
    pub(crate) user: UserId,
}

#[derive(Clone)]
pub(crate) struct AuthClient {
    client: Client,
    me: Url,
    cookie_name: &'static str,
}

#[derive(Debug)]
pub(crate) enum AuthError {
    Unauthorized,
    Unavailable,
}

#[derive(Deserialize)]
struct Profile {
    id: String,
}

impl AuthClient {
    pub(crate) fn new(base: &str, secure: bool) -> Result<Self, &'static str> {
        let mut me = Url::parse(base).map_err(|_| "USER_SERVICE_URL must be a URL")?;
        if me.scheme() != "http" && me.scheme() != "https"
            || me.host_str().is_none()
            || me.path() != "/"
            || me.query().is_some()
            || me.fragment().is_some()
            || !me.username().is_empty()
            || me.password().is_some()
        {
            return Err("USER_SERVICE_URL must be an HTTP origin without credentials or path");
        }
        me.set_path("/me");
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()
            .map_err(|_| "could not configure auth HTTP client")?;
        Ok(Self {
            client,
            me,
            cookie_name: if secure {
                "__Host-user_session"
            } else {
                "user_session"
            },
        })
    }

    pub(crate) fn cookie(&self, headers: &HeaderMap) -> Result<String, AuthError> {
        let mut found = None;
        for line in headers.get_all(header::COOKIE) {
            for item in line
                .to_str()
                .map_err(|_| AuthError::Unauthorized)?
                .split(';')
            {
                if let Some((key, value)) = item.trim().split_once('=')
                    && key == self.cookie_name
                {
                    if found.is_some()
                        || value.len() != 64
                        || !value.bytes().all(|b| b.is_ascii_hexdigit())
                    {
                        return Err(AuthError::Unauthorized);
                    }
                    found = Some(value.to_owned());
                }
            }
        }
        found.ok_or(AuthError::Unauthorized)
    }

    pub(crate) async fn authenticate(&self, cookie: &str) -> Result<UserId, AuthError> {
        let response = self
            .client
            .get(self.me.clone())
            .header(
                reqwest::header::COOKIE,
                format!("{}={cookie}", self.cookie_name),
            )
            .send()
            .await
            .map_err(|_| AuthError::Unavailable)?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AuthError::Unauthorized);
        }
        if response.status() != reqwest::StatusCode::OK {
            return Err(AuthError::Unavailable);
        }
        if response.content_length().is_some_and(|len| len > 4096) {
            return Err(AuthError::Unavailable);
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| AuthError::Unavailable)?;
            if bytes.len() + chunk.len() > 4096 {
                return Err(AuthError::Unavailable);
            }
            bytes.extend_from_slice(&chunk);
        }
        let profile: Profile =
            serde_json::from_slice(&bytes).map_err(|_| AuthError::Unavailable)?;
        UserId::new(profile.id).map_err(|_| AuthError::Unavailable)
    }

    pub(crate) async fn recheck(&self, session: &Session) -> Result<(), AuthError> {
        let user = self.authenticate(&session.cookie).await?;
        if user == session.user {
            Ok(())
        } else {
            Err(AuthError::Unauthorized)
        }
    }
}
