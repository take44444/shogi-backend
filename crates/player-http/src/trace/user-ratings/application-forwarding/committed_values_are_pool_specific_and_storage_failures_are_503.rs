crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実 Axum ルーターとアプリケーション層。偽 repository の固定値と失敗を使用する。",
        reason: "成功値と 2 種類の失敗がアプリケーション層を通る。HTTP は失敗を同じ 503 に変換する。",
        relationship: crate::trace_support::Relationship::Exercises,
        also: &[crate::trace_support::Evidence { feature: "user-ratings", contract: "http-pools", scope: "実 Axum ルーターとアプリケーション層。偽 repository の固定値と失敗を使用する。", relationship: crate::trace_support::Relationship::Asserts, reason: "片方に 1516、もう片方に null を持つ応答を確認する。" }, crate::trace_support::Evidence { feature: "user-ratings", contract: "rating-state", scope: "実 Axum ルーターとアプリケーション層。偽 repository の固定値と失敗を使用する。", relationship: crate::trace_support::Relationship::Exercises, reason: "RatingState::from_parts(Some(1516), 1) による評価済み状態を使う。" }, crate::trace_support::Evidence { feature: "user-ratings", contract: "http-unavailable", scope: "実 Axum ルーターとアプリケーション層。偽 repository の固定値と失敗を使用する。", relationship: crate::trace_support::Relationship::Asserts, reason: "偽 repository の Error::Unavailable と Error::Corrupt の双方で 503 と同一の JSON エラーを比較する。" }, crate::trace_support::Evidence { feature: "user-ratings", contract: "http-cache", scope: "実 Axum ルーターとアプリケーション層。偽 repository の固定値と失敗を使用する。", relationship: crate::trace_support::Relationship::Asserts, reason: "成功と保存層の 2 種類の失敗で、共有ヘルパーが no-store を確認する。" }],
    };
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
}
