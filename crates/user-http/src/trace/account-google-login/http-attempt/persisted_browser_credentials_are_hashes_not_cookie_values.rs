crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実ルーターと PostgreSQL の保存行を比較する。本人確認は偽 provider。",
        reason: "start が保存する state_hash・browser_hash が生値と異なり SHA-256 に一致すること、callback が 303 を返しセッションもハッシュで保存することを確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "account-session", contract: "session-storage", scope: "偽 provider の実ルーターから PostgreSQL に保存された credential hash を読む。", relationship: crate::trace_support::Relationship::Asserts, reason: "保存済み token_hash が Cookie の生値と異なり、その SHA-256 と一致することを確認する。" }],
    };
#[tokio::test]
async fn persisted_browser_credentials_are_hashes_not_cookie_values() {
    use sea_orm::{
        ConnectionTrait, Database,
        sea_query::{Alias, Query},
    };
    let database = pg_test_support::TestDatabase::new().await;
    let store = Arc::new(PostgresAccounts::connect(database.url()).await.unwrap());
    let app = app(store, Arc::new(Provider::default()));
    let raw = Database::connect(database.url()).await.unwrap();
    let (state, browser) = begin(&app).await;
    let browser_value = browser.split_once('=').unwrap().1;
    let attempt = raw
        .query_one(
            Query::select()
                .columns([Alias::new("state_hash"), Alias::new("browser_hash")])
                .from(Alias::new("login_attempts")),
        )
        .await
        .unwrap()
        .unwrap();
    let saved_state: String = attempt.try_get("", "state_hash").unwrap();
    let saved_browser: String = attempt.try_get("", "browser_hash").unwrap();
    assert_ne!(saved_state, state);
    assert_ne!(saved_browser, browser_value);
    assert_eq!(saved_state, hash(&state));
    assert_eq!(saved_browser, hash(browser_value));
    let response = call(
        &app,
        "GET",
        &format!("/auth/google/callback?state={state}&code=valid"),
        Some(&browser),
        None,
        "",
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = response_cookie(&response, "user_session");
    let token = cookie.split_once('=').unwrap().1;
    let session = raw
        .query_one(
            Query::select()
                .column(Alias::new("token_hash"))
                .from(Alias::new("sessions")),
        )
        .await
        .unwrap()
        .unwrap();
    let saved: String = session.try_get("", "token_hash").unwrap();
    assert_ne!(saved, token);
    assert_eq!(saved, hash(token));
}
}
