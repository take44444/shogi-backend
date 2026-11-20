use super::*;

#[tokio::test]
async fn exact_replay_returns_original_receipt_after_finish_and_conflicts_are_rejected() {
    let server = TestServer::start().await;
    let mut client = server.client().await;
    create(&mut client, "replay").await;
    let first = command("replay", "first", 0, proto::Player::Black, "7g7f");
    let receipt = client.make_move(first.clone()).await.unwrap().into_inner();
    client
        .make_move(command("replay", "second", 1, proto::Player::White, "3c3d"))
        .await
        .unwrap();
    let resignation = resign("replay", "end", 2, proto::Player::White);
    let finish = client
        .resign(resignation.clone())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(client.make_move(first).await.unwrap().into_inner(), receipt);
    assert_eq!(
        client.resign(resignation).await.unwrap().into_inner(),
        finish
    );
    for changed in [
        command("replay", "first", 1, proto::Player::Black, "7g7f"),
        command("replay", "first", 0, proto::Player::White, "7g7f"),
        command("replay", "first", 0, proto::Player::Black, "2g2f"),
        command("replay", "end", 2, proto::Player::White, "3c3d"),
    ] {
        assert_eq!(
            client.make_move(changed).await.unwrap_err().code(),
            Code::AlreadyExists
        );
    }
    assert_eq!(
        client
            .resign(resign("replay", "first", 0, proto::Player::Black))
            .await
            .unwrap_err()
            .code(),
        Code::AlreadyExists
    );
    assert_eq!(snapshot(&mut client, "replay").await.seq, 3);
    drop(client);
    server.shutdown().await;
}

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
