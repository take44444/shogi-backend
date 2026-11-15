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
include!(
    "trace/public-api-documents/contract-user-schema/document_identifies_this_service_without_dependency_attribution.rs"
);
include!(
    "trace/public-api-documents/contract-user-schema/document_has_all_routes_real_schemas_and_cookie_security_alternatives.rs"
);
include!(
    "trace/public-api-documents/contract-user-schema/document_distinguishes_json_errors_and_framework_text_rejections.rs"
);
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
include!(
    "trace/public-api-documents/user-runtime/runtime_document_equals_offline_document_without_storage_or_provider_calls.rs"
);
include!(
    "trace/account-profile/http-rename/runtime_rejection_status_and_media_match_documented_contract.rs"
);
