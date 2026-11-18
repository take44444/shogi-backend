use super::*;

#[traceability::targets(game_application::GameService::snapshot)]
#[tokio::test]
async fn closing_the_pool_makes_the_game_service_unavailable() {
    let database = TestDatabase::new().await;
    let (store, games) = setup(database.url()).await;
    assert_eq!(games.snapshot(&id()).await.unwrap().seq, 0);

    store.close().await.unwrap();

    assert_eq!(games.snapshot(&id()).await, Err(Error::Unavailable));
}
