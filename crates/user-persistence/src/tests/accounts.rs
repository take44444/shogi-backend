#![cfg(test)]

use super::*;
fn identity() -> VerifiedIdentity {
    VerifiedIdentity {
        issuer: "https://accounts.google.com".into(),
        subject: "subject".into(),
        initial_name: DisplayName::new("Player".into()).unwrap(),
    }
}
fn session(hash: &str, expiry: i64) -> NewSession {
    NewSession {
        token_hash: hash.into(),
        expires_at: expiry,
    }
}
fn attempt(state: &str, expiry: i64) -> LoginAttempt {
    LoginAttempt {
        state_hash: state.into(),
        browser_hash: "browser-hash".into(),
        nonce: "nonce".into(),
        pkce_verifier: "verifier".into(),
        expires_at: expiry,
    }
}
#[path = "accounts_login.rs"]
mod account_google_login_account_session_transaction;
#[path = "accounts_attempts.rs"]
mod account_google_login_attempt_consumption;
#[path = "accounts_identity.rs"]
mod account_profile_existing_identity;

#[path = "accounts_schema.rs"]
mod startup_schema_isolation_contract_account_schema;
