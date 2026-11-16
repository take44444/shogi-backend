use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "呼び出されると panic するリポジトリと外部認証プロバイダーを使う、プロセス内の HTTPS 用・開発用ユーザールーター。",
        reason: "プロセス内の HTTPS 用・開発用ルーターが、セキュリティヘッダー付きでオフライン生成と同じ JSON を返すことを確認する。リポジトリと外部認証プロバイダーは、呼ばれると panic する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn runtime_document_equals_offline_document_without_storage_or_provider_calls() {
    for secure in [true, false] {
        let response = offline_router(secure)
            .oneshot(
                Request::builder()
                    .uri("/openapi.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
        assert_eq!(
            response.headers()[header::X_CONTENT_TYPE_OPTIONS],
            "nosniff"
        );
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), document());
    }
}
}
