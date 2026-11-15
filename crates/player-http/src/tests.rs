use super::*;
use axum::{body::Body, http::Request};
use game_application::{GameId as WriteId, MakeMove, Participants, RequestId, Resign};
use game_persistence::SeaOrmStore;
use game_record_persistence::PostgresGameRecords;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

async fn get(app: Router, path: &str) -> (StatusCode, Value) {
    let response = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}
include!(
    "trace/public-record-detail/application-forwarding/anonymous_list_is_metadata_only_and_detail_contains_only_public_ordered_actions.rs"
);
include!(
    "trace/public-record-detail/http-boundary/query_boundary_rejects_bad_filters_bounds_and_cursors_before_storage.rs"
);
struct Failing(Error);
#[async_trait::async_trait]
impl game_record_application::RecordRepository for Failing {
    async fn search(&self, _: &Search) -> Result<Page, Error> {
        Err(self.0)
    }
    async fn detail(&self, _: &GameId) -> Result<Detail, Error> {
        Err(self.0)
    }
}
include!(
    "trace/public-record-detail/application-forwarding/bounded_detail_and_storage_errors_are_generic.rs"
);
