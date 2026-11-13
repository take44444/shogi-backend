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
#[tokio::test]
async fn anonymous_unknown_user_has_two_explicit_null_pools_and_zero_counts() {
    let (status, body) = get(Ok(UserRatingSnapshot::default()), "/users/unknown/ratings").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        serde_json::json!({"ten_minute_30":{"rating":null,"completed_rated_games":0},"byoyomi_30":{"rating":null,"completed_rated_games":0}})
    );
}
#[tokio::test]
async fn committed_values_are_pool_specific_and_storage_failures_are_503() {
    let snapshot = UserRatingSnapshot {
        ten_minute_30: RatingState::from_parts(Some(1516), 1).unwrap(),
        byoyomi_30: RatingState::Unrated,
    };
    let (status, body) = get(Ok(snapshot), "/users/unknown/ratings").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ten_minute_30"]["rating"], 1516);
    assert!(body["byoyomi_30"]["rating"].is_null());
    for error in [Error::Unavailable, Error::Corrupt] {
        assert_eq!(
            get(Err(error), "/users/unknown/ratings").await,
            (
                StatusCode::SERVICE_UNAVAILABLE,
                serde_json::json!({"error":"unavailable"})
            )
        );
    }
}
#[tokio::test]
async fn invalid_user_ids_are_rejected_before_storage() {
    for path in [
        "/users/has%20space/ratings",
        "/users/%E3%80%80/ratings",
        "/users/%FF/ratings",
    ] {
        let app = router(GameRecords::new(Arc::new(NoRecords)), unused_ratings());
        let response = app
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
    }
}
#[test]
fn rating_openapi_contains_required_null_rating_and_both_named_pools() {
    let d = serde_json::to_value(openapi()).unwrap();
    let route = &d["paths"]["/users/{user_id}/ratings"]["get"];
    assert_eq!(route["parameters"][0]["name"], "user_id");
    assert_eq!(route["parameters"][0]["required"], true);
    for status in ["200", "400", "503"] {
        assert!(route["responses"][status].is_object());
    }
    assert!(route.get("security").is_none());
    assert_eq!(
        d["components"]["schemas"]["RatingsDto"]["required"],
        serde_json::json!(["ten_minute_30", "byoyomi_30"])
    );
    assert_eq!(
        d["components"]["schemas"]["PoolRatingDto"]["required"],
        serde_json::json!(["rating", "completed_rated_games"])
    );
    assert_eq!(
        d["components"]["schemas"]["PoolRatingDto"]["properties"]["rating"]["type"],
        serde_json::json!(["integer", "null"])
    );
}
