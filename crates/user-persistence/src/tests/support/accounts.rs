use super::*;
pub(super) fn identity() -> VerifiedIdentity {
    VerifiedIdentity {
        issuer: "https://accounts.google.com".into(),
        subject: "subject".into(),
        initial_name: DisplayName::new("Player".into()).unwrap(),
    }
}
pub(super) fn session(hash: &str, expiry: i64) -> NewSession {
    NewSession {
        token_hash: hash.into(),
        expires_at: expiry,
    }
}
pub(super) fn attempt(state: &str, expiry: i64) -> LoginAttempt {
    LoginAttempt {
        state_hash: state.into(),
        browser_hash: "browser-hash".into(),
        nonce: "nonce".into(),
        pkce_verifier: "verifier".into(),
        expires_at: expiry,
    }
}
