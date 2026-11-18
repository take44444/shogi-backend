use super::*;

#[tokio::test]
async fn tampered_and_duplicate_session_cookies_never_authenticate() {
    let database = pg_test_support::TestDatabase::new().await;
    let store = Arc::new(PostgresAccounts::connect(database.url()).await.unwrap());
    let app = app(store, Arc::new(Provider::default()));
    let session = login(&app).await;
    let mut changed = session.clone();
    changed.pop();
    changed.push(if session.ends_with('a') { 'b' } else { 'a' });
    assert_eq!(
        call(&app, "GET", "/me", Some(&changed), None, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "GET",
            "/me",
            Some(&format!("{session}; {session}")),
            None,
            ""
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}
