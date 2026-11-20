#![cfg(test)]

use super::*;
use chrono::{Duration, Utc};
use openidconnect::{
    AccessToken, Audience, AuthUrl, EmptyAdditionalClaims, EmptyExtraTokenFields, JsonWebKeyId,
    JsonWebKeySetUrl, PrivateSigningKey, ResponseTypes, StandardClaims, SubjectIdentifier,
    core::{
        CoreIdToken, CoreIdTokenClaims, CoreIdTokenFields, CoreJsonWebKeySet,
        CoreJwsSigningAlgorithm, CoreResponseType, CoreRsaPrivateSigningKey,
        CoreSubjectIdentifierType, CoreTokenResponse, CoreTokenType,
    },
};

fn key() -> CoreRsaPrivateSigningKey {
    CoreRsaPrivateSigningKey::from_pem(
        include_str!("google_test_rsa.pem"),
        Some(JsonWebKeyId::new("test-key".into())),
    )
    .unwrap()
}
fn client() -> DiscoveredClient {
    client_with_token(None)
}
fn client_with_token(token: Option<openidconnect::TokenUrl>) -> DiscoveredClient {
    let metadata = CoreProviderMetadata::new(
        IssuerUrl::new(GOOGLE_ISSUER.into()).unwrap(),
        AuthUrl::new("https://accounts.google.com/auth".into()).unwrap(),
        JsonWebKeySetUrl::new("https://accounts.google.com/keys".into()).unwrap(),
        vec![ResponseTypes::new(vec![CoreResponseType::Code])],
        vec![CoreSubjectIdentifierType::Public],
        vec![CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256],
        openidconnect::EmptyAdditionalProviderMetadata {},
    )
    .set_jwks(CoreJsonWebKeySet::new(vec![key().as_verification_key()]))
    .set_token_endpoint(token);
    CoreClient::from_provider_metadata(
        metadata,
        ClientId::new("client-id".into()),
        Some(ClientSecret::new("secret".into())),
    )
}
fn claims() -> CoreIdTokenClaims {
    CoreIdTokenClaims::new(
        IssuerUrl::new(GOOGLE_ISSUER.into()).unwrap(),
        vec![Audience::new("client-id".into())],
        Utc::now() + Duration::seconds(300),
        Utc::now(),
        StandardClaims::new(SubjectIdentifier::new("stable-subject".into())),
        EmptyAdditionalClaims {},
    )
    .set_nonce(Some(Nonce::new("expected-nonce".into())))
}
fn response(claims: CoreIdTokenClaims, access: &str) -> CoreTokenResponse {
    let token = CoreIdToken::new(
        claims,
        &key(),
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
        Some(&AccessToken::new("access-token".into())),
        None,
    )
    .unwrap();
    CoreTokenResponse::new(
        AccessToken::new(access.into()),
        CoreTokenType::Bearer,
        CoreIdTokenFields::new(Some(token), EmptyExtraTokenFields {}),
    )
}
fn verify(claims: CoreIdTokenClaims) -> Result<VerifiedIdentity, Error> {
    verify_response(
        &client(),
        "client-id",
        response(claims, "access-token"),
        "expected-nonce".into(),
    )
}
#[path = "google_claims.rs"]
mod account_google_login_verified_claims;

#[path = "google_authorization.rs"]
mod account_google_login_provider_authorization;

#[path = "google_exchange.rs"]
mod account_google_login_provider_exchange;
