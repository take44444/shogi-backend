use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。",
        reason: "実際の接続プールを閉じると、キャッシュ済みサービスのスナップショット取得が Unavailable を返す。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn closing_the_pool_makes_the_game_service_unavailable() {
    let database = TestDatabase::new().await;
    let (store, games) = setup(database.url()).await;
    assert_eq!(games.snapshot(&id()).await.unwrap().seq, 0);

    store.close().await.unwrap();

    assert_eq!(games.snapshot(&id()).await, Err(Error::Unavailable));
}
}
