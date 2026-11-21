use super::*;
use crate::record_route_support::unused_ratings;

#[tokio::test]
async fn query_boundary_rejects_bad_filters_bounds_and_cursors_before_storage() {
    let repository = Arc::new(Failing(game_record_application::Error::Corrupt));
    let app = router(GameRecords::new(repository), unused_ratings());
    for path in [
        "/game-records",
        "/game-records?from=0&to=0",
        "/game-records?from=-1&to=2",
        "/game-records?from=0&to=31622400000001",
        "/game-records?from=0&to=100&limit=0",
        "/game-records?from=0&to=100&limit=101",
        "/game-records?from=0&to=100&min_ply=2&max_ply=1",
        "/game-records?from=0&to=100&min_ply=18446744073709551615",
        "/game-records?from=0&to=100&participant=bad%20id",
        "/game-records?from=0&to=100&winner=alice",
        "/game-records?from=0&to=100&rule=other",
        "/game-records?from=0&to=100&reason=other",
        "/game-records?from=0&to=100&cursor=garbage",
        "/game-records?from=0&to=100&unknown=1",
        "/game-records?from=0&to=100&limit=1&limit=2",
        "/game-records/bad%20id",
    ] {
        assert_eq!(
            get(app.clone(), path).await,
            (
                StatusCode::BAD_REQUEST,
                serde_json::json!({"error":"invalid_query"})
            ),
            "{path}"
        );
    }
    assert_eq!(
        get(app, "/game-records?from=0&to=100").await.0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
}
