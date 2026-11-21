use super::*;

#[tokio::test]
async fn generated_tcp_client_restores_database_across_two_server_restarts() {
    let database = pg_test_support::TestDatabase::new().await;
    let url = database.url().to_string();
    let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
    let server =
        TestServer::start_service(GameService::new(store.clone()), store.clone(), &url).await;
    let mut client = server.client().await;
    create(&mut client, "restart").await;
    let first = command("restart", "first", 0, proto::Player::Black, "7g7f");
    let original = client.make_move(first.clone()).await.unwrap().into_inner();
    let ongoing = snapshot(&mut client, "restart").await;
    drop(client);
    server.shutdown().await;
    store.close().await.unwrap();
    let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
    let server =
        TestServer::start_service(GameService::new(store.clone()), store.clone(), &url).await;
    let mut client = server.client().await;
    assert_eq!(snapshot(&mut client, "restart").await, ongoing);
    assert_eq!(
        client.make_move(first.clone()).await.unwrap().into_inner(),
        original
    );
    client
        .make_move(command("restart", "white", 1, proto::Player::White, "3c3d"))
        .await
        .unwrap();
    let end = resign("restart", "end", 2, proto::Player::Black);
    let terminal = client.resign(end.clone()).await.unwrap().into_inner();
    let finished = snapshot(&mut client, "restart").await;
    drop(client);
    server.shutdown().await;
    store.close().await.unwrap();
    let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
    let server = TestServer::start_service(GameService::new(store.clone()), store, &url).await;
    let mut client = server.client().await;
    assert_eq!(snapshot(&mut client, "restart").await, finished);
    assert_eq!(
        client.make_move(first).await.unwrap().into_inner(),
        original
    );
    assert_eq!(client.resign(end).await.unwrap().into_inner(), terminal);
    drop(client);
    server.shutdown().await;
}
