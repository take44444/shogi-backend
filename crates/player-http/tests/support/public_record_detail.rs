use super::*;

pub(super) use axum::{body::Body, http::Request};
pub(super) use game_application::{GameId as WriteId, MakeMove, Participants, RequestId, Resign};
pub(super) use game_persistence::SeaOrmStore;
pub(super) use game_record_persistence::PostgresGameRecords;
pub(super) use http_body_util::BodyExt;
pub(super) use serde_json::Value;
pub(super) use tower::ServiceExt;

pub(super) async fn get(app: Router, path: &str) -> (StatusCode, Value) {
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
pub(super) struct Failing(pub(super) Error);
#[async_trait::async_trait]
impl game_record_application::RecordRepository for Failing {
    async fn search(&self, _: &Search) -> Result<Page, Error> {
        Err(self.0)
    }
    async fn detail(&self, _: &GameId) -> Result<Detail, Error> {
        Err(self.0)
    }
}
