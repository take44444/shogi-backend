use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/grpc-snapshot", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。", reason: "実際の接続プールを閉じると、キャッシュ済みサービスのスナップショット取得が Unavailable を返す。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-application"], fake: &[] }] };
#[tokio::test]
async fn closing_the_pool_makes_the_game_service_unavailable() {
    let database = TestDatabase::new().await;
    let (store, games) = setup(database.url()).await;
    assert_eq!(games.snapshot(&id()).await.unwrap().seq, 0);

    store.close().await.unwrap();

    assert_eq!(games.snapshot(&id()).await, Err(Error::Unavailable));
}
}
