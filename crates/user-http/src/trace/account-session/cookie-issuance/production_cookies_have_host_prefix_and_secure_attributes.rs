crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "HTTPS BrowserConfig の start 応答。ログイン Cookie のみを確認し、callback とセッション Cookie は通らない。",
        reason: "本番のログイン Cookie が __Host-user_login で始まり Secure・HttpOnly・SameSite=Lax・Path=/・Max-Age=600 を持ち、Domain がないことを確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
