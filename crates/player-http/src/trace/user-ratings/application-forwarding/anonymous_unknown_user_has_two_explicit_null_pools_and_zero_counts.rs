crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実 Axum ルーターとアプリケーション層。偽 RatingRepository を使い、DB は使わない。",
        reason: "実ルーターから UserRatings::get を通り、偽 repository は受けた ID が unknown であることを確認する。",
        relationship: crate::trace_support::Relationship::Exercises,
        also: &[crate::trace_support::Evidence { feature: "user-ratings", contract: "http-pools", scope: "実 Axum ルーターとアプリケーション層。偽 RatingRepository を使い、DB は使わない。", relationship: crate::trace_support::Relationship::Asserts, reason: "認証情報なしの未知ユーザーへの 200 応答と、両プールの null・0 を JSON 全体で比較する。" }, crate::trace_support::Evidence { feature: "user-ratings", contract: "rating-state", scope: "実 Axum ルーターとアプリケーション層。偽 RatingRepository を使い、DB は使わない。", relationship: crate::trace_support::Relationship::Exercises, reason: "既定の Unrated を HTTP に変換する。" }, crate::trace_support::Evidence { feature: "user-ratings", contract: "http-cache", scope: "実 Axum ルーターとアプリケーション層。偽 RatingRepository を使い、DB は使わない。", relationship: crate::trace_support::Relationship::Asserts, reason: "成功応答で共有ヘルパーが no-store を確認する。" }],
    };
#[tokio::test]
async fn anonymous_unknown_user_has_two_explicit_null_pools_and_zero_counts() {
    let (status, body) = get(Ok(UserRatingSnapshot::default()), "/users/unknown/ratings").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        serde_json::json!({"ten_minute_30":{"rating":null,"completed_rated_games":0},"byoyomi_30":{"rating":null,"completed_rated_games":0}})
    );
}
}
