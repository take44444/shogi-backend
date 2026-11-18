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
#[path = "account-session-transaction/tests.rs"]
mod account_google_login_account_session_transaction;
#[path = "attempt-consumption/tests.rs"]
mod account_google_login_attempt_consumption;
#[path = "../account-profile/existing-identity/tests.rs"]
mod account_profile_existing_identity;

#[path = "../startup-schema-isolation/contract-account-schema/tests.rs"]
mod startup_schema_isolation_contract_account_schema;
