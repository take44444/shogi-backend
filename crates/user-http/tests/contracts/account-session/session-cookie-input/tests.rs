use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "user-http/frontend-get-me", relationship: crate::trace_support::Relationship::Asserts, scope: "公開ルーター、実アカウント保存、テスト用 IdentityProvider。実 Google には接続しない。", reason: "HTTP 応答と Cookie、保存結果またはログアウト後の拒否を直接検査する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["user-http", "user-persistence"], fake: &["google"] }] };
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
}
