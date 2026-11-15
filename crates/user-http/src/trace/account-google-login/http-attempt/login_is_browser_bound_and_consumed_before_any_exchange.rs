crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実 Axum ルーターと PostgreSQL。交換回数を数える偽 IdentityProvider を使い、実 Google には接続しない。",
        reason: "ブラウザー Cookie の欠落・不一致では交換回数 0、成功後の再利用は 401、失敗コードでの交換後も再試行不可であることを確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn login_is_browser_bound_and_consumed_before_any_exchange() {
    let database = pg_test_support::TestDatabase::new().await;
    let store = Arc::new(PostgresAccounts::connect(database.url()).await.unwrap());
    let provider = Arc::new(Provider::default());
    let app = app(store, provider.clone());
    let (state, browser) = begin(&app).await;
    let callback = format!("/auth/google/callback?state={state}&code=valid");
    assert_eq!(
        call(&app, "GET", &callback, None, None, "").await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "GET",
            &callback,
            Some(&format!("user_login={}", random())),
            None,
            ""
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(provider.exchanges.load(Ordering::SeqCst), 0);
    assert_eq!(
        call(&app, "GET", &callback, Some(&browser), None, "")
            .await
            .status(),
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        call(&app, "GET", &callback, Some(&browser), None, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(provider.exchanges.load(Ordering::SeqCst), 1);
    let (state, browser) = begin(&app).await;
    let bad = format!("/auth/google/callback?state={state}&code=bad");
    assert_eq!(
        call(&app, "GET", &bad, Some(&browser), None, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let retry = format!("/auth/google/callback?state={state}&code=valid");
    assert_eq!(
        call(&app, "GET", &retry, Some(&browser), None, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(provider.exchanges.load(Ordering::SeqCst), 2);
}
}
