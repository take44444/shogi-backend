use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "ローカル HTTP token endpoint とテスト RSA 署名を使う exchange_code。discovery とブラウザールートは通らない。",
        reason: "ローカル HTTP token endpoint に送る grant_type・code・verifier・redirect_uri と、署名済み応答からの subject を確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "account-google-login", contract: "verified-claims", scope: "ローカル HTTP token endpoint とテスト RSA 署名を使う exchange_code。discovery とブラウザールートは通らない。", relationship: crate::trace_support::Relationship::Asserts, reason: "正しく署名した応答を実際の検証関数に通して stable-subject を返すことを確認する。" }],
    };
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
        expires_at: crate::now() + 600,
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
}
