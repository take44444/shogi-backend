use super::*;

fn grant(provider: &VerificationProvider) -> (String, LoginAttempt) {
    let auth = provider.authorize();
    let url = url::Url::parse(&auth.url).unwrap();
    let id = url
        .query_pairs()
        .find(|(key, _)| key == "request")
        .unwrap()
        .1
        .into_owned();
    let callback =
        url::Url::parse(&provider.select(&id, VerificationActor::Alice).unwrap()).unwrap();
    let code = callback
        .query_pairs()
        .find(|(key, _)| key == "code")
        .unwrap()
        .1
        .into_owned();
    (
        code,
        LoginAttempt {
            state_hash: hash(&auth.state),
            browser_hash: "browser".into(),
            nonce: auth.nonce,
            pkce_verifier: auth.pkce_verifier,
            expires_at: now() + TTL,
        },
    )
}
#[tokio::test]
async fn grants_are_single_use_and_bind_every_authorization_field() {
    let provider = VerificationProvider::new("http://localhost:8080".into());
    let (code, attempt) = grant(&provider);
    let identity = provider
        .exchange(code.clone(), copy_attempt(&attempt))
        .await
        .unwrap();
    assert_eq!(
        (identity.issuer.as_str(), identity.subject.as_str()),
        (ISSUER, "alice")
    );
    assert!(provider.exchange(code, attempt).await.is_err());
    for field in ["state", "nonce", "verifier", "expiry"] {
        let (code, mut attempt) = grant(&provider);
        match field {
            "state" => attempt.state_hash = "wrong".into(),
            "nonce" => attempt.nonce = "wrong".into(),
            "verifier" => attempt.pkce_verifier = "wrong".into(),
            _ => attempt.expires_at = now(),
        }
        assert!(
            provider
                .exchange(code.clone(), copy_attempt(&attempt))
                .await
                .is_err()
        );
        assert!(provider.exchange(code, attempt).await.is_err());
    }
    let (_, attempt) = grant(&provider);
    assert!(provider.exchange("invalid".into(), attempt).await.is_err());
}
#[tokio::test]
async fn expired_grants_and_requests_and_restarts_require_fresh_login() {
    let provider = VerificationProvider::new("http://localhost:8080".into());
    let auth = provider.authorize();
    let id = url::Url::parse(&auth.url)
        .unwrap()
        .query_pairs()
        .next()
        .unwrap()
        .1
        .into_owned();
    provider
        .pending
        .lock()
        .unwrap()
        .requests
        .get_mut(&id)
        .unwrap()
        .expires_at = now();
    assert!(provider.select(&id, VerificationActor::Bob).is_err());
    let (code, attempt) = grant(&provider);
    provider
        .pending
        .lock()
        .unwrap()
        .codes
        .get_mut(&code)
        .unwrap()
        .expires_at = now();
    assert!(provider.exchange(code, attempt).await.is_err());
    let (code, attempt) = grant(&provider);
    let restarted = VerificationProvider::new(provider.origin.clone());
    assert!(restarted.exchange(code, attempt).await.is_err());
}
#[test]
fn requests_and_grants_share_a_bounded_budget() {
    let provider = VerificationProvider::new("http://localhost:8080".into());
    for _ in 0..CAPACITY + 5 {
        provider.authorize();
    }
    assert_eq!(provider.pending.lock().unwrap().requests.len(), CAPACITY);
    let id = provider
        .pending
        .lock()
        .unwrap()
        .requests
        .keys()
        .next()
        .unwrap()
        .clone();
    provider.select(&id, VerificationActor::Bob).unwrap();
    assert!(provider.select(&id, VerificationActor::Alice).is_err());
    provider.authorize();
    let pending = provider.pending.lock().unwrap();
    assert_eq!(pending.requests.len() + pending.codes.len(), CAPACITY);
}

fn copy_attempt(attempt: &LoginAttempt) -> LoginAttempt {
    LoginAttempt {
        state_hash: attempt.state_hash.clone(),
        browser_hash: attempt.browser_hash.clone(),
        nonce: attempt.nonce.clone(),
        pkce_verifier: attempt.pkce_verifier.clone(),
        expires_at: attempt.expires_at,
    }
}
