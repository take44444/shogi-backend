#![cfg(test)]

use super::*;
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

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
#[path = "openapi_schema.rs"]
mod public_api_documents_contract_player_schema;

struct UnusedRecords;
#[async_trait::async_trait]
impl game_record_application::RecordRepository for UnusedRecords {
    async fn search(&self, _: &Search) -> Result<Page, Error> {
        panic!("document must not query storage")
    }
    async fn detail(&self, _: &GameId) -> Result<Detail, Error> {
        panic!("document must not query storage")
    }
}
#[path = "openapi_runtime.rs"]
mod public_api_documents_player_runtime;
