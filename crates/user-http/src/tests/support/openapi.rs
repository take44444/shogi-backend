use super::*;
pub(super) use async_trait::async_trait;
pub(super) use axum::{body::Body, http::Request};
pub(super) use http_body_util::BodyExt;
pub(super) use serde_json::{Value, json};
pub(super) use tower::ServiceExt;
pub(super) use user_application::VerifiedIdentity;

pub(super) fn document() -> Value {
    serde_json::to_value(openapi()).unwrap()
}
pub(super) fn assert_local_refs_resolve(document: &Value, value: &Value) {
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

pub(super) struct Unused;
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
pub(super) fn offline_router(secure: bool) -> Router {
    let browser = if secure {
        BrowserConfig::new("https://users.example.com", false).unwrap()
    } else {
        BrowserConfig::new("http://localhost:8080", true).unwrap()
    };
    router(Arc::new(Unused), Arc::new(Unused), browser)
}
