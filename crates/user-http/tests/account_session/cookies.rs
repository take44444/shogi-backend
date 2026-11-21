use super::*;

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
