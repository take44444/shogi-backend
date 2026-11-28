use async_trait::async_trait;
use axum::{
    Form, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use user_application::{DisplayName, Error, LoginAttempt, VerifiedIdentity};
use user_http::{Authorization, IdentityProvider};

const TTL: i64 = 60;
const CAPACITY: usize = 256;
const ISSUER: &str = "urn:shogi:verification";

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum VerificationActor {
    Alice,
    Bob,
}
impl VerificationActor {
    fn name(self) -> &'static str {
        match self {
            Self::Alice => "Alice",
            Self::Bob => "Bob",
        }
    }
    fn subject(self) -> &'static str {
        match self {
            Self::Alice => "alice",
            Self::Bob => "bob",
        }
    }
}
struct AuthorizationRequest {
    state: String,
    nonce: String,
    verifier: String,
    expires_at: i64,
}
struct CodeGrant {
    actor: VerificationActor,
    state_hash: String,
    nonce: String,
    verifier: String,
    expires_at: i64,
}
#[derive(Default)]
struct PendingAuthorizations {
    requests: HashMap<String, AuthorizationRequest>,
    codes: HashMap<String, CodeGrant>,
}
impl PendingAuthorizations {
    fn prune(&mut self, now: i64) {
        self.requests.retain(|_, request| request.expires_at > now);
        self.codes.retain(|_, grant| grant.expires_at > now);
    }
}
pub(super) struct VerificationProvider {
    origin: String,
    pending: Mutex<PendingAuthorizations>,
}
impl VerificationProvider {
    pub(super) fn new(origin: String) -> Self {
        Self {
            origin,
            pending: Mutex::new(PendingAuthorizations::default()),
        }
    }
    fn select(&self, request: &str, actor: VerificationActor) -> Result<String, Error> {
        let mut pending = self.pending.lock().expect("authorization lock");
        pending.prune(now());
        let request = pending
            .requests
            .remove(request)
            .ok_or(Error::Unauthorized)?;
        let code = random();
        pending.codes.insert(
            code.clone(),
            CodeGrant {
                actor,
                state_hash: hash(&request.state),
                nonce: request.nonce,
                verifier: request.verifier,
                expires_at: request.expires_at,
            },
        );
        let mut callback = url::Url::parse(&format!("{}/auth/google/callback", self.origin))
            .expect("validated origin");
        callback
            .query_pairs_mut()
            .append_pair("state", &request.state)
            .append_pair("code", &code);
        Ok(callback.into())
    }
}
#[async_trait]
impl IdentityProvider for VerificationProvider {
    fn authorize(&self) -> Authorization {
        let state = random();
        let nonce = random();
        let verifier = random();
        let id = random();
        let mut pending = self.pending.lock().expect("authorization lock");
        pending.prune(now());
        if pending.requests.len() + pending.codes.len() < CAPACITY {
            pending.requests.insert(
                id.clone(),
                AuthorizationRequest {
                    state: state.clone(),
                    nonce: nonce.clone(),
                    verifier: verifier.clone(),
                    expires_at: now() + TTL,
                },
            );
        }
        Authorization {
            url: format!("{}/__verify/authorize?request={id}", self.origin),
            state,
            nonce,
            pkce_verifier: verifier,
        }
    }
    async fn exchange(
        &self,
        code: String,
        attempt: LoginAttempt,
    ) -> Result<VerifiedIdentity, Error> {
        let mut pending = self.pending.lock().expect("authorization lock");
        pending.prune(now());
        let grant = pending.codes.remove(&code).ok_or(Error::Unauthorized)?;
        if grant.state_hash != attempt.state_hash
            || grant.nonce != attempt.nonce
            || grant.verifier != attempt.pkce_verifier
            || attempt.expires_at <= now()
        {
            return Err(Error::Unauthorized);
        }
        Ok(VerifiedIdentity {
            issuer: ISSUER.into(),
            subject: grant.actor.subject().into(),
            initial_name: DisplayName::new(grant.actor.name().into()).expect("fixed display name"),
        })
    }
}
pub(super) fn router(provider: Arc<VerificationProvider>) -> Router {
    Router::new()
        .route("/__verify/authorize", get(choice).post(select))
        .with_state(provider)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestQuery {
    request: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    request: String,
    actor: VerificationActor,
}
async fn choice(
    State(provider): State<Arc<VerificationProvider>>,
    Query(query): Query<RequestQuery>,
) -> Response {
    let mut pending = provider.pending.lock().expect("authorization lock");
    pending.prune(now());
    let Some((registered_id, _)) = pending.requests.get_key_value(&query.request) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let page = format!(
        "<!doctype html><html><head><title>Verification login</title></head><body><h1>Verification login</h1><p>Disposable local accounts</p><form method='post' action='/__verify/authorize'><input type='hidden' name='request' value='{}'><button name='actor' value='alice'>Alice</button><button name='actor' value='bob'>Bob</button></form></body></html>",
        registered_id
    );
    (
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::REFERRER_POLICY, "same-origin"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; form-action 'self'; frame-ancestors 'none'",
            ),
        ],
        Html(page),
    )
        .into_response()
}
async fn select(
    State(provider): State<Arc<VerificationProvider>>,
    headers: HeaderMap,
    Form(selection): Form<Selection>,
) -> Response {
    if headers.get(header::ORIGIN).and_then(|h| h.to_str().ok()) != Some(provider.origin.as_str())
        || headers
            .get("sec-fetch-site")
            .is_some_and(|h| h != "same-origin")
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    match provider.select(&selection.request, selection.actor) {
        Ok(callback) => (
            [
                (header::CACHE_CONTROL, "no-store"),
                (header::REFERRER_POLICY, "no-referrer"),
            ],
            Redirect::to(&callback),
        )
            .into_response(),
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
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

#[cfg(test)]
mod tests;
