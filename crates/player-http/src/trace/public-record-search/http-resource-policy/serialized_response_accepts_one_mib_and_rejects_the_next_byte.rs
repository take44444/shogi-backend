crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shared response helper with exact serialized byte boundaries; no real record page.",
        reason: "Direct response-helper checks accept exactly one MiB and return 422 with a generic error for the next byte.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn serialized_response_accepts_one_mib_and_rejects_the_next_byte() {
    let response = respond(Ok("x".repeat(1_048_574)));
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(body.len(), 1_048_576);

    let response = respond(Ok("x".repeat(1_048_575)));
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({"error":"history_too_long"})
    );
}
}
