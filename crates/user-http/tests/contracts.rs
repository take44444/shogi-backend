use axum::{
    Router,
    http::{StatusCode, header},
    response::Response,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use user_application::{DisplayName, Error, LoginAttempt};
use user_http::{Authorization, BrowserConfig, GOOGLE_ISSUER, IdentityProvider, router};

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
#[path = "contracts/account-google-login/support/browser.rs"]
mod tests;
