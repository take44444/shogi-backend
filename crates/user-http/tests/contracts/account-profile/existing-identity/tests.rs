use super::*;

#[tokio::test]
async fn registration_profile_update_csrf_logout_and_restart() {
    let database = pg_test_support::TestDatabase::new().await;
    let url = database.url();
    let store = Arc::new(PostgresAccounts::connect(url).await.unwrap());
    let provider = Arc::new(Provider::default());
    let first = app(store.clone(), provider.clone());
    assert_eq!(
        call(&first, "GET", "/me", None, None, "").await.status(),
        StatusCode::UNAUTHORIZED
    );
    let session = login(&first).await;
    let profile = json(call(&first, "GET", "/me", Some(&session), None, "").await).await;
    assert_eq!(profile["display_name"], "Google Player");
    assert!(uuid::Uuid::parse_str(profile["id"].as_str().unwrap()).is_ok());
    assert_eq!(
        call(
            &first,
            "PATCH",
            "/me",
            Some(&session),
            None,
            r#"{"display_name":"new"}"#
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &first,
            "PATCH",
            "/me",
            Some(&session),
            Some("https://attacker.invalid"),
            r#"{"display_name":"new"}"#
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &first,
            "PATCH",
            "/me",
            Some(&session),
            Some("http://localhost:8080"),
            r#"{"display_name":"   "}"#
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let renamed = json(
        call(
            &first,
            "PATCH",
            "/me",
            Some(&session),
            Some("http://localhost:8080"),
            r#"{"display_name":"New Name"}"#,
        )
        .await,
    )
    .await;
    assert_eq!(renamed["display_name"], "New Name");
    drop(first);
    store.close().await.unwrap();
    let restarted_store = Arc::new(PostgresAccounts::connect(url).await.unwrap());
    let restarted = app(restarted_store.clone(), provider);
    assert_eq!(
        json(call(&restarted, "GET", "/me", Some(&session), None, "").await).await,
        renamed
    );
    let second_session = login(&restarted).await;
    assert_eq!(
        json(call(&restarted, "GET", "/me", Some(&second_session), None, "").await).await,
        renamed
    );
    assert_eq!(
        call(&restarted, "POST", "/auth/logout", Some(&session), None, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &restarted,
            "POST",
            "/auth/logout",
            Some(&session),
            Some("http://localhost:8080"),
            ""
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&restarted, "GET", "/me", Some(&session), None, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&restarted, "GET", "/me", Some(&second_session), None, "")
            .await
            .status(),
        StatusCode::OK
    );
    restarted_store.close().await.unwrap();
    let final_store = Arc::new(PostgresAccounts::connect(url).await.unwrap());
    assert_eq!(
        call(
            &app(final_store, Arc::new(Provider::default())),
            "GET",
            "/me",
            Some(&session),
            None,
            ""
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}
