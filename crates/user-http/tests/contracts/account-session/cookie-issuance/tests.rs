use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "user-http/frontend-get-auth-google-start", relationship: crate::trace_support::Relationship::Asserts, scope: "公開ルーター、実アカウント保存、テスト用 IdentityProvider。実 Google には接続しない。", reason: "HTTP 応答と Cookie、保存結果またはログアウト後の拒否を直接検査する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["user-http", "user-persistence"], fake: &["google"] }] };
#[tokio::test]
async fn production_cookies_have_host_prefix_and_secure_attributes() {
    let database = pg_test_support::TestDatabase::new().await;
    let store = Arc::new(PostgresAccounts::connect(database.url()).await.unwrap());
    let app = router(
        store,
        Arc::new(Provider::default()),
        BrowserConfig::new("https://users.example.com", false).unwrap(),
    );
    let r = call(&app, "GET", "/auth/google/start", None, None, "").await;
    let c = r.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(c.starts_with("__Host-user_login="));
    for required in [
        "Secure",
        "HttpOnly",
        "SameSite=Lax",
        "Path=/",
        "Max-Age=600",
    ] {
        assert!(c.contains(required));
    }
    assert!(!c.contains("Domain="));
}
}
