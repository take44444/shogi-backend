use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "In-process HTTPS/development user routers with panic-on-use repository and provider.",
        reason: "HTTPS and development in-process routers return equal offline JSON with security headers; repository and provider panic if used.",
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
