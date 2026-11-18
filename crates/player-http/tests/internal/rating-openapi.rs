use super::*;

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
