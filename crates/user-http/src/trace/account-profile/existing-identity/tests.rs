use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実 Axum ルーターと PostgreSQL、偽 provider。プロフィール取得・CSRF・名前更新・再接続・同じ本人識別の再ログインを通る。",
        reason: "New Name に変更した後、初期名 Google Player を返す偽 provider で再ログインしてもプロフィール全体が維持されることを確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "account-profile", contract: "profile-response", scope: "実 Axum ルーターと PostgreSQL、偽 provider。プロフィール取得・CSRF・名前更新・再接続・同じ本人識別の再ログインを通る。", relationship: crate::trace_support::Relationship::Asserts, reason: "初回プロフィールの Google Player と UUID、名前変更後・再接続後・同じ本人識別の次回ログイン後の JSON 全体の一致を確認する。" }, crate::trace_support::Evidence { feature: "account-profile", contract: "http-rename", scope: "実 Axum ルーターと PostgreSQL、偽 provider。プロフィール取得・CSRF・名前更新・再接続・同じ本人識別の再ログインを通る。", relationship: crate::trace_support::Relationship::Asserts, reason: "有効な session で Origin 欠落と hostile Origin を 401、空白だけの名前を 400 とし、同一 Origin の有効な名前は応答と保存内容に反映する。" }, crate::trace_support::Evidence { feature: "account-profile", contract: "profile-update", scope: "実 Axum ルーターと PostgreSQL、偽 provider。プロフィール取得・CSRF・名前更新・再接続・同じ本人識別の再ログインを通る。", relationship: crate::trace_support::Relationship::Asserts, reason: "実ルーターで New Name に更新し、store の再接続後の GET /me が同じプロフィールとなることを確認する。" }, crate::trace_support::Evidence { feature: "account-session", contract: "session-cookie-input", scope: "実 Axum ルーターと PostgreSQL。偽 provider で二つのセッションを作り、logout と再接続後の失効を確認する。", relationship: crate::trace_support::Relationship::Asserts, reason: "Cookie のない GET /me が 401 となることを確認し、有効な Cookie でプロフィール取得を通る。" }, crate::trace_support::Evidence { feature: "account-session", contract: "session-storage", scope: "実 Axum ルーターと PostgreSQL。偽 provider で二つのセッションを作り、logout と再接続後の失効を確認する。", relationship: crate::trace_support::Relationship::Asserts, reason: "実ルーターでログアウトした token が store の再接続後も 401 で、別 token は 200 であることを確認する。" }, crate::trace_support::Evidence { feature: "account-session", contract: "http-logout", scope: "実 Axum ルーターと PostgreSQL。偽 provider で二つのセッションを作り、logout と再接続後の失効を確認する。", relationship: crate::trace_support::Relationship::Asserts, reason: "Origin 欠落の 401、正しい Origin の 204、対象 token の 401 と別 token の 200 を確認する。" }],
    };
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
}
