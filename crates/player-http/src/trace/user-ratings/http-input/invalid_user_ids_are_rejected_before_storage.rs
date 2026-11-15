crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実 Axum ルーター。呼び出されると panic する偽 repository を使用する。",
        reason: "空白・全角空白・不正 UTF-8 のパスを 400 で拒否し、呼ばれると panic する repository に到達しない。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn invalid_user_ids_are_rejected_before_storage() {
    for path in [
        "/users/has%20space/ratings",
        "/users/%E3%80%80/ratings",
        "/users/%FF/ratings",
    ] {
        let app = router(GameRecords::new(Arc::new(NoRecords)), unused_ratings());
        let response = app
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
    }
}
}
