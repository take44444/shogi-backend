use super::*;
use crate::record_route_support::unused_ratings;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "player-http/frontend-get-users-user-id-ratings", relationship: crate::trace_support::Relationship::Asserts, scope: "公開 API で構築した実ルーター。repository の実装は準備コードで実 DB または fake を選択する。", reason: "公開ルーターの応答本文・ステータス・ヘッダーを検証する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["player-http"], fake: &[] }] };
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
}
