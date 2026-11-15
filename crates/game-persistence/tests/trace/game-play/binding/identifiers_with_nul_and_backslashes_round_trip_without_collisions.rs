#[tokio::test]
async fn identifiers_with_nul_and_backslashes_round_trip_without_collisions() {
    let database = TestDatabase::new().await;
    let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
    let games = pg_test_support::GameFixture::new(store.clone(), database.url());
    let black = user("black\0\\name");
    let white = user("white\\0name");
    let game = GameId::new("game\0\\name").unwrap();
    let literal = GameId::new("game\\0\\name").unwrap();
    for id in [&game, &literal] {
        games
            .seed(
                id.clone(),
                if *id == game {
                    Participants::new(black.clone(), white.clone()).unwrap()
                } else {
                    Participants::new(user("literal-black"), user("literal-white")).unwrap()
                },
            )
            .await
            .unwrap();
    }
    let command = MakeMove {
        request_id: RequestId::new("request\0\\name").unwrap(),
        expected_seq: 0,
        user_id: black.clone(),
        movement: "7g7f".parse().unwrap(),
    };
    let accepted = games.make_move(&game, command.clone()).await.unwrap();
    store.close().await.unwrap();
    let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
    let restored = pg_test_support::GameFixture::new(store.clone(), database.url());
    assert_eq!(
        restored.make_move(&game, command.clone()).await.unwrap(),
        accepted
    );
    let record = store.load_game(&game).await.unwrap();
    assert_eq!(record.id, game);
    assert_eq!(
        record.participants,
        Participants::new(black, white).unwrap()
    );
    assert_eq!(record.journal[0].request.request_id, command.request_id);
    assert_eq!(restored.snapshot(&literal).await.unwrap().seq, 0);
}
