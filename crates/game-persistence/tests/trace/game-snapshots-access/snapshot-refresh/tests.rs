use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport.",
        reason: "Closing the real pool makes cached service snapshot return Unavailable.",
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
