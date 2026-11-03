use super::*;
use async_trait::async_trait;
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use user_application::VerifiedIdentity;

fn document() -> Value {
    serde_json::to_value(openapi()).unwrap()
}
fn assert_local_refs_resolve(document: &Value, value: &Value) {
    match value {
        Value::Object(fields) => {
            if let Some(Value::String(reference)) = fields.get("$ref") {
                assert!(
                    document
                        .pointer(reference.strip_prefix('#').unwrap())
                        .is_some(),
                    "{reference}"
                );
            }
            for value in fields.values() {
                assert_local_refs_resolve(document, value);
            }
        }
        Value::Array(values) => {
            for value in values {
                assert_local_refs_resolve(document, value);
            }
        }
        _ => {}
    }
}
#[test]
fn document_identifies_this_service_without_dependency_attribution() {
    let d = document();
    assert_eq!(d["info"]["title"], env!("CARGO_PKG_NAME"));
    assert_eq!(d["info"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(d["info"].get("contact").is_none());
    assert!(d["info"].get("license").is_none());
}
#[test]
fn document_has_all_routes_real_schemas_and_cookie_security_alternatives() {
    let d = document();
    assert_eq!(
        d["paths"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "/auth/google/callback",
            "/auth/google/start",
            "/auth/logout",
            "/me",
            "/openapi.json"
        ]
    );
    for (path, methods) in [
        ("/auth/google/callback", vec!["get"]),
        ("/auth/google/start", vec!["get"]),
        ("/auth/logout", vec!["post"]),
        ("/me", vec!["get", "patch"]),
        ("/openapi.json", vec!["get"]),
    ] {
        assert_eq!(
            d["paths"][path]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            methods
        );
    }
    let schemes = &d["components"]["securitySchemes"];
    for (scheme, name) in [
        ("session", "__Host-user_session"),
        ("development_session", "user_session"),
        ("login", "__Host-user_login"),
        ("development_login", "user_login"),
    ] {
        assert_eq!(schemes[scheme]["type"], "apiKey");
        assert_eq!(schemes[scheme]["in"], "cookie");
        assert_eq!(schemes[scheme]["name"], name);
    }
    assert_eq!(
        d["paths"]["/auth/google/callback"]["get"]["security"],
        json!([{"login":[]},{"development_login":[]}])
    );
    for method in ["get", "patch"] {
        assert_eq!(
            d["paths"]["/me"][method]["security"],
            json!([{"session":[]},{"development_session":[]}])
        );
    }
    for (path, method) in [
        ("/auth/google/start", "get"),
        ("/auth/logout", "post"),
        ("/openapi.json", "get"),
    ] {
        assert!(d["paths"][path][method].get("security").is_none());
    }
    let params = d["paths"]["/auth/google/callback"]["get"]["parameters"]
        .as_array()
        .unwrap();
    assert_eq!(params.len(), 3);
    assert_eq!(
        params.iter().find(|p| p["name"] == "state").unwrap()["required"],
        true
    );
    for (path, method) in [("/me", "patch"), ("/auth/logout", "post")] {
        let params = d["paths"][path][method]["parameters"].as_array().unwrap();
        let origin = params.iter().find(|p| p["name"] == "Origin").unwrap();
        assert_eq!(origin["required"], true);
        assert_eq!(origin["in"], "header");
        let fetch = params
            .iter()
            .find(|p| p["name"] == "Sec-Fetch-Site")
            .unwrap();
        assert_eq!(fetch["required"], false);
        assert!(
            fetch["description"]
                .as_str()
                .unwrap()
                .contains("same-origin")
        );
    }
    for path in ["/auth/google/start", "/auth/google/callback"] {
        let redirect = &d["paths"][path]["get"]["responses"]["303"];
        assert!(redirect.get("content").is_none());
        assert!(redirect["headers"]["Location"].is_object());
        assert!(redirect["headers"]["Set-Cookie"].is_object());
    }
    assert!(
        d["paths"]["/auth/logout"]["post"]["responses"]["204"]
            .get("content")
            .is_none()
    );
    let schemas = &d["components"]["schemas"];
    assert_eq!(
        schemas["ProfileBody"]["required"],
        json!(["id", "display_name", "created_at"])
    );
    assert!(
        schemas["ProfileBody"]["properties"]["created_at"]["description"]
            .as_str()
            .unwrap()
            .contains("seconds")
    );
    assert_eq!(schemas["Rename"]["required"], json!(["display_name"]));
    assert_eq!(schemas["Rename"]["additionalProperties"], false);
    assert!(
        schemas["Rename"]["properties"]["display_name"]
            .get("maxLength")
            .is_none()
    );
    assert_eq!(
        schemas["ErrorCode"]["enum"],
        json!([
            "authentication_required",
            "invalid_input",
            "service_unavailable"
        ])
    );
    assert_local_refs_resolve(&d, &d);
}
#[test]
fn document_distinguishes_json_errors_and_framework_text_rejections() {
    let d = document();
    let rename = &d["paths"]["/me"]["patch"]["responses"];
    assert_eq!(
        rename
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["200", "400", "401", "413", "415", "422", "503"]
    );
    for status in ["400", "401", "503"] {
        assert_eq!(
            rename[status]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ErrorBody"
        );
    }
    for status in ["400", "413", "415", "422"] {
        assert_eq!(
            rename[status]["content"]["text/plain"]["schema"]["type"],
            "string"
        );
    }
    assert!(
        d["paths"]["/auth/google/callback"]["get"]["responses"]["400"]["content"]["text/plain"]
            .is_object()
    );
}
struct Unused;
#[async_trait]
impl AccountRepository for Unused {
    async fn save_attempt(&self, _: LoginAttempt, _: i64) -> Result<(), Error> {
        panic!("unused storage")
    }
    async fn consume_attempt(&self, _: &str, _: &str, _: i64) -> Result<LoginAttempt, Error> {
        panic!("unused storage")
    }
    async fn login(&self, _: VerifiedIdentity, _: NewSession, _: i64) -> Result<Profile, Error> {
        panic!("unused storage")
    }
    async fn authenticate(&self, _: &str, _: i64) -> Result<Profile, Error> {
        panic!("unused storage")
    }
    async fn rename(&self, _: &str, _: DisplayName, _: i64) -> Result<Profile, Error> {
        panic!("unused storage")
    }
    async fn logout(&self, _: &str) -> Result<(), Error> {
        panic!("unused storage")
    }
}
#[async_trait]
impl IdentityProvider for Unused {
    fn authorize(&self) -> Authorization {
        panic!("unused provider")
    }
    async fn exchange(&self, _: String, _: LoginAttempt) -> Result<VerifiedIdentity, Error> {
        panic!("unused provider")
    }
}
fn offline_router(secure: bool) -> Router {
    let browser = if secure {
        BrowserConfig::new("https://users.example.com", false).unwrap()
    } else {
        BrowserConfig::new("http://localhost:8080", true).unwrap()
    };
    router(Arc::new(Unused), Arc::new(Unused), browser)
}
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
