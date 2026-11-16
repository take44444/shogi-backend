use super::*;
use crate::record_route_support::unused_ratings;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "プロセス内のルーターと、失敗を返すテスト用リポジトリ。不正な形式の詳細取得 ID も含む。",
        reason: "復号後に空白を含む ID は、失敗を返すリポジトリが 500 を返す前に 400 となる。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "public-record-search", contract: "query-boundary", scope: "失敗を返すテスト用リポジトリを使う実 Axum ルーター。不正な事例で保存層へ到達する前の拒否を確認する。", relationship: crate::trace_support::Relationship::Asserts, reason: "各不正リクエストが 400、invalid_query、no-store を返すことを確認する。有効なリクエストは同じ失敗するリポジトリへ到達して 500 を返す。" }, crate::trace_support::Evidence { feature: "public-record-search", contract: "application-forwarding", scope: "失敗を返すテスト用リポジトリを使う実 Axum ルーター。不正な事例で保存層へ到達する前の拒否を確認する。", relationship: crate::trace_support::Relationship::Exercises, reason: "有効なクエリーはこのメソッドを通り、リポジトリの失敗に到達する。" }, crate::trace_support::Evidence { feature: "public-record-search", contract: "http-resource-policy", scope: "失敗を返すテスト用リポジトリを使う実 Axum ルーター。不正な事例で保存層へ到達する前の拒否を確認する。", relationship: crate::trace_support::Relationship::Asserts, reason: "不正な検索リクエストについて、詳細を伏せた 400 の JSON 本文と no-store を検証する。破損したリポジトリに到達する有効なリクエストについては、500 と no-store を検証し、本文は確認しない。" }],
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
