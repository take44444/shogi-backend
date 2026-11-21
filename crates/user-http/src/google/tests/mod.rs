use super::*;

#[path = "support/provider.rs"]
mod provider_support;
use provider_support::{
    Audience, CoreTokenResponse, Duration, Utc, claims, client, client_with_token, response, verify,
};

mod authorization;
mod claims;
mod exchange;
