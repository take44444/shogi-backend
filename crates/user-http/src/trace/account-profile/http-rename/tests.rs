use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実ルーターで JSON 抽出と入力拒否を確認する。repository と provider は呼ばれると panic する偽物で、成功する rename は通らない。",
        reason: "不正 JSON 400、本文過大 413、Content-Type 欠落 415、未知フィールドを持つ構造 422 の text/plain と、空白名 400 の application/json を確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "account-session", contract: "http-logout", scope: "依存先が呼ばれると panic する実ルーター。Cookie のない同一 Origin logout と入力拒否を確認する。", relationship: crate::trace_support::Relationship::Asserts, reason: "セッション Cookie のない同一 Origin の POST が 204 と空の開発用セッション Cookie を返すことを確認する。" }, crate::trace_support::Evidence { feature: "account-session", contract: "cookie-issuance", scope: "依存先が呼ばれると panic する実ルーター。Cookie のない同一 Origin logout と入力拒否を確認する。", relationship: crate::trace_support::Relationship::Asserts, reason: "開発用の logout Cookie が user_session=; で始まることを確認する。" }, crate::trace_support::Evidence { feature: "public-api-documents", contract: "contract-user-schema", scope: "In-process user router checks selected documented response status/media and no-store.", relationship: crate::trace_support::Relationship::Asserts, reason: "Runtime framework text rejections and application JSON rejections are checked against documented status/media and no-store." }],
    };
#[tokio::test]
async fn runtime_rejection_status_and_media_match_documented_contract() {
    let app = offline_router(false);
    let oversized = format!(r#"{{"display_name":"{}"}}"#, "x".repeat(2_097_152));
    for (uri, method, content_type, body, status, media) in [
        ("/auth/google/callback", "GET", None, "", 400, "text/plain"),
        (
            "/me",
            "PATCH",
            Some("application/json"),
            "{",
            400,
            "text/plain",
        ),
        (
            "/me",
            "PATCH",
            Some("application/json"),
            &oversized,
            413,
            "text/plain",
        ),
        (
            "/me",
            "PATCH",
            None,
            r#"{"display_name":"name"}"#,
            415,
            "text/plain",
        ),
        (
            "/me",
            "PATCH",
            Some("application/json"),
            r#"{"unexpected":true}"#,
            422,
            "text/plain",
        ),
        (
            "/me",
            "PATCH",
            Some("application/json"),
            r#"{"display_name":"  "}"#,
            400,
            "application/json",
        ),
        ("/me", "GET", None, "", 401, "application/json"),
    ] {
        let mut request = Request::builder()
            .uri(uri)
            .method(method)
            .header(header::ORIGIN, "http://localhost:8080");
        if let Some(content_type) = content_type {
            request = request.header(header::CONTENT_TYPE, content_type);
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::from(body.to_owned())).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), status, "{method} {uri}");
        assert_eq!(
            response.headers()[header::CONTENT_TYPE]
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap(),
            media
        );
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert!(
            document()["paths"][uri][method.to_ascii_lowercase()]["responses"][status.to_string()]
                ["content"][media]
                .is_object()
        );
    }
    let response = app
        .oneshot(
            Request::builder()
                .uri("/auth/logout")
                .method("POST")
                .header(header::ORIGIN, "http://localhost:8080")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .starts_with("user_session=;")
    );
}
}
