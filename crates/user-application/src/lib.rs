//! Account, identity, login attempt, and session storage boundary.
use async_trait::async_trait;
pub use user_id::{InvalidId, UserId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub id: UserId,
    pub display_name: DisplayName,
    pub created_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayName(String);
impl DisplayName {
    pub fn new(value: String) -> Result<Self, Error> {
        let trimmed = value.trim();
        if trimmed.is_empty()
            || trimmed.chars().count() > 80
            || trimmed.chars().any(char::is_control)
        {
            return Err(Error::InvalidInput);
        }
        Ok(Self(trimmed.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub struct LoginAttempt {
    pub state_hash: String,
    pub browser_hash: String,
    pub nonce: String,
    pub pkce_verifier: String,
    pub expires_at: i64,
}

pub struct VerifiedIdentity {
    pub issuer: String,
    pub subject: String,
    pub initial_name: DisplayName,
}

pub struct NewSession {
    pub token_hash: String,
    pub expires_at: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("authentication required")]
    Unauthorized,
    #[error("invalid input")]
    InvalidInput,
    #[error("storage unavailable")]
    Unavailable,
    #[error("incompatible account database")]
    Corrupt,
}

#[async_trait]
pub trait AccountRepository: Send + Sync {
    async fn save_attempt(&self, attempt: LoginAttempt, now: i64) -> Result<(), Error>;
    /// Atomically consumes exactly one unexpired attempt bound to the browser.
    async fn consume_attempt(
        &self,
        state_hash: &str,
        browser_hash: &str,
        now: i64,
    ) -> Result<LoginAttempt, Error>;
    /// Resolves an issuer/subject and creates a session in the same transaction.
    async fn login(
        &self,
        identity: VerifiedIdentity,
        session: NewSession,
        now: i64,
    ) -> Result<Profile, Error>;
    async fn authenticate(&self, token_hash: &str, now: i64) -> Result<Profile, Error>;
    async fn rename(&self, token_hash: &str, name: DisplayName, now: i64)
    -> Result<Profile, Error>;
    async fn logout(&self, token_hash: &str) -> Result<(), Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_names_are_trimmed_and_bound_in_unicode_characters() {
        assert_eq!(
            DisplayName::new("  将棋  ".into()).unwrap().as_str(),
            "将棋"
        );
        assert!(DisplayName::new("界".repeat(80)).is_ok());
        for name in [
            "".into(),
            "  ".into(),
            "界".repeat(81),
            "a\nb".into(),
            "a\0b".into(),
        ] {
            assert_eq!(DisplayName::new(name), Err(Error::InvalidInput));
        }
    }
}
