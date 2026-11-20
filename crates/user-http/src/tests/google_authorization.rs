use super::*;

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
