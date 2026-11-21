use axum::{
    Router,
    http::{StatusCode, header},
    response::Response,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use user_application::{DisplayName, Error, LoginAttempt};
use user_http::{Authorization, BrowserConfig, GOOGLE_ISSUER, IdentityProvider, router};

#[path = "support/browser.rs"]
mod browser_support;
use browser_support::{
    Ordering, PostgresAccounts, Provider, app, begin, call, hash, json, login, random,
    response_cookie,
};

#[path = "account_profile/lifecycle.rs"]
mod account_profile;
mod account_session;
#[path = "configuration/browser.rs"]
mod browser_config;
#[path = "google_login/browser_attempts.rs"]
mod google_login;
