use super::*;
use crate::record_route_support::unused_ratings;

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
