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
        include_str!("test-rsa.pem"),
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
#[test]
fn signed_valid_token_resolves_only_issuer_and_subject() {
    let identity = verify(claims()).unwrap();
    assert_eq!(identity.issuer, GOOGLE_ISSUER);
    assert_eq!(identity.subject, "stable-subject");
    assert_eq!(identity.initial_name.as_str(), "Player");
}
#[test]
fn signed_wrong_issuer_audience_nonce_expiry_or_azp_is_rejected() {
    let invalid = [
        claims().set_issuer(IssuerUrl::new("https://attacker.invalid".into()).unwrap()),
        claims().set_audiences(vec![Audience::new("another-client".into())]),
        claims().set_nonce(Some(Nonce::new("another-nonce".into()))),
        claims().set_nonce(None),
        claims().set_expiration(Utc::now() - Duration::seconds(1)),
        claims().set_authorized_party(Some(ClientId::new("another-client".into()))),
        claims().set_audiences(vec![
            Audience::new("client-id".into()),
            Audience::new("untrusted-client".into()),
        ]),
    ];
    for c in invalid {
        assert!(matches!(verify(c), Err(Error::Unauthorized)));
    }
    assert!(verify(claims().set_authorized_party(Some(ClientId::new("client-id".into())))).is_ok());
}
#[test]
fn substituted_access_token_and_tampered_signature_are_rejected() {
    assert!(matches!(
        verify_response(
            &client(),
            "client-id",
            response(claims(), "substituted"),
            "expected-nonce".into()
        ),
        Err(Error::Unauthorized)
    ));
    let mut value = serde_json::to_value(response(claims(), "access-token")).unwrap();
    let encoded = value["id_token"].as_str().unwrap();
    let mut parts = encoded.split('.').map(str::to_owned).collect::<Vec<_>>();
    let original = parts[2].remove(0);
    parts[2].insert(0, if original == 'a' { 'b' } else { 'a' });
    value["id_token"] = serde_json::Value::String(parts.join("."));
    let tampered: CoreTokenResponse = serde_json::from_value(value).unwrap();
    assert!(matches!(
        verify_response(&client(), "client-id", tampered, "expected-nonce".into()),
        Err(Error::Unauthorized)
    ));
}

#[test]
fn authorization_uses_openid_profile_state_nonce_and_s256_pkce() {
    let provider = GoogleProvider {
        client: client(),
        http: reqwest::Client::new(),
        client_id: "client-id".into(),
        client_secret: "secret".into(),
        callback: "https://users.example.com/auth/google/callback".into(),
    };
    let authorization = provider.authorize();
    let url = url::Url::parse(&authorization.url).unwrap();
    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(query["scope"], "openid profile");
    assert_eq!(query["response_type"], "code");
    assert_eq!(query["state"], authorization.state);
    assert_eq!(query["nonce"], authorization.nonce);
    assert_eq!(query["code_challenge_method"], "S256");
    assert_eq!(
        query["code_challenge"],
        PkceCodeChallenge::from_code_verifier_sha256(&PkceCodeVerifier::new(
            authorization.pkce_verifier
        ))
        .as_str()
    );
    assert_ne!(authorization.state, provider.authorize().state);
}

#[tokio::test]
async fn code_exchange_sends_pkce_and_verifies_signed_token_over_http() {
    use axum::{
        Json, Router,
        extract::{Form, State},
        routing::post,
    };
    use std::sync::{Arc, Mutex};
    type Fields = std::collections::HashMap<String, String>;
    #[derive(Clone)]
    struct Mock {
        fields: Arc<Mutex<Option<Fields>>>,
        response: serde_json::Value,
    }
    async fn endpoint(
        State(mock): State<Mock>,
        Form(fields): Form<Fields>,
    ) -> Json<serde_json::Value> {
        *mock.fields.lock().unwrap() = Some(fields);
        Json(mock.response)
    }
    let mock = Mock {
        fields: Arc::new(Mutex::new(None)),
        response: serde_json::to_value(response(claims(), "access-token")).unwrap(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let token_url = format!("http://{}/token", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/token", post(endpoint))
        .with_state(mock.clone());
    let serving = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = client_with_token(Some(openidconnect::TokenUrl::new(token_url).unwrap()))
        .set_redirect_uri(
            RedirectUrl::new("https://users.example.com/auth/google/callback".into()).unwrap(),
        );
    let attempt = LoginAttempt {
        state_hash: "state-hash".into(),
        browser_hash: "browser-hash".into(),
        nonce: "expected-nonce".into(),
        pkce_verifier: "a".repeat(43),
        expires_at: super::super::now() + 600,
    };
    let identity = exchange_code(
        &client,
        &reqwest::Client::new(),
        "client-id",
        "auth-code".into(),
        attempt,
    )
    .await
    .unwrap();
    assert_eq!(identity.subject, "stable-subject");
    let fields = mock.fields.lock().unwrap().clone().unwrap();
    assert_eq!(fields["grant_type"], "authorization_code");
    assert_eq!(fields["code"], "auth-code");
    assert_eq!(fields["code_verifier"], "a".repeat(43));
    assert_eq!(
        fields["redirect_uri"],
        "https://users.example.com/auth/google/callback"
    );
    serving.abort();
}
