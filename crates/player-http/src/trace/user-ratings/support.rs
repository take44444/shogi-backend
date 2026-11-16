#![cfg(test)]

use super::*;
use axum::{body::Body, http::Request};
use game_record_application::ratings::{RatingRepository, RatingState, UserRatingSnapshot};
use http_body_util::BodyExt;
use tower::ServiceExt;

struct Ratings(Result<UserRatingSnapshot, Error>);
#[async_trait::async_trait]
impl RatingRepository for Ratings {
    async fn ratings(&self, user: &UserId) -> Result<UserRatingSnapshot, Error> {
        assert_eq!(user.as_str(), "unknown");
        self.0
    }
}
struct NoRecords;
#[async_trait::async_trait]
impl game_record_application::RecordRepository for NoRecords {
    async fn search(&self, _: &Search) -> Result<Page, Error> {
        panic!("rating route must not read records")
    }
    async fn detail(&self, _: &GameId) -> Result<Detail, Error> {
        panic!("rating route must not read records")
    }
}
async fn get(
    result: Result<UserRatingSnapshot, Error>,
    path: &str,
) -> (StatusCode, serde_json::Value) {
    let app = router(
        GameRecords::new(Arc::new(NoRecords)),
        UserRatings::new(Arc::new(Ratings(result))),
    );
    let response = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}
#[path = "application-forwarding/tests.rs"]
mod user_ratings_application_forwarding;

#[path = "http-input/tests.rs"]
mod user_ratings_http_input;
#[path = "http-openapi/tests.rs"]
mod user_ratings_http_openapi;
