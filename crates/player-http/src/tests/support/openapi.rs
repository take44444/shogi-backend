use super::*;
pub(super) use axum::{body::Body, http::Request};
pub(super) use http_body_util::BodyExt;
pub(super) use serde_json::{Value, json};
pub(super) use tower::ServiceExt;

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

pub(super) struct UnusedRecords;
#[async_trait::async_trait]
impl game_record_application::RecordRepository for UnusedRecords {
    async fn search(&self, _: &Search) -> Result<Page, Error> {
        panic!("document must not query storage")
    }
    async fn detail(&self, _: &GameId) -> Result<Detail, Error> {
        panic!("document must not query storage")
    }
}
