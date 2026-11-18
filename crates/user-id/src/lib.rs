//! Shared internal user identifiers. An identifier is never an authentication credential.
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidId;
impl fmt::Display for InvalidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ID must contain 1..=128 bytes and no whitespace")
    }
}
impl Error for InvalidId {}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UserId(String);
impl UserId {
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidId> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 || value.chars().any(char::is_whitespace) {
            return Err(InvalidId);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
#[path = "tests/account-profile/user-id/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../scripts/traceability_model.rs"]
mod trace_support;

#[cfg(test)]
mod trace_metadata;
