use super::*;

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
