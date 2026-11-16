use super::*;
use crate::record_route_support::unused_ratings;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "呼び出されると panic するリポジトリを使う、プロセス内の公開対局ルーター。",
        reason: "認証なしのルーター応答がオフライン生成の JSON と一致し、呼び出されると panic する対局・レートのリポジトリに到達しないことを確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn runtime_document_equals_offline_document_without_storage_access() {
    let response = router(GameRecords::new(Arc::new(UnusedRecords)), unused_ratings())
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
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(), document());
}
}
