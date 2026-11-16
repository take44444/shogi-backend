use super::*;
use crate::record_route_support::unused_ratings;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "In-process router and failing fake repository; includes malformed detail ID.",
        reason: "An ID containing a decoded space returns 400 before the failing repository can return 500.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "public-record-search", contract: "query-boundary", scope: "Real Axum router with a failing fake repository; invalid cases distinguish rejection before storage.", relationship: crate::trace_support::Relationship::Asserts, reason: "Each invalid request returns 400 and invalid_query with no-store; a valid request reaches the same failing repository and returns 500." }, crate::trace_support::Evidence { feature: "public-record-search", contract: "application-forwarding", scope: "Real Axum router with a failing fake repository; invalid cases distinguish rejection before storage.", relationship: crate::trace_support::Relationship::Exercises, reason: "The valid query traverses this method to a repository failure." }, crate::trace_support::Evidence { feature: "public-record-search", contract: "http-resource-policy", scope: "Real Axum router with a failing fake repository; invalid cases distinguish rejection before storage.", relationship: crate::trace_support::Relationship::Asserts, reason: "Invalid search requests assert generic 400 JSON bodies and no-store. A valid request reaching the corrupt repository asserts 500 status and no-store, without checking its body." }],
    };
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
}
