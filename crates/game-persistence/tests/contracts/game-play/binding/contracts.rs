use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/grpc-command", relationship: crate::trace_support::Relationship::Unreviewed, scope: "製品の公開 API を呼ぶ結合テスト。個々の assertion と契約の対応は未レビュー。", reason: "宣言した実提供元の境界を利用する。契約の各条件を直接検証するかは未レビュー。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-application"], fake: &[] }] };
#[tokio::test]
async fn distinct_ids_create_games_without_accounts_and_bindings_are_immutable() {
    {
        let database = TestDatabase::new().await;
        let url = database.url().to_string();
        let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
        let service = pg_test_support::GameFixture::new(store, &url);
        assert_eq!(
            Participants::new(user("black"), user("black")),
            Err(Error::InvalidParticipants)
        );
        let before = service.seed(id(), participants()).await.unwrap();
        assert_eq!(
            service
                .seed(
                    id(),
                    Participants::new(user("white"), user("black")).unwrap()
                )
                .await,
            Err(Error::AlreadyExists)
        );
        assert_eq!(service.snapshot(&id()).await.unwrap(), before);
        let accepted = service
            .make_move(&id(), movement("first", 0, "black", "7g7f"))
            .await
            .unwrap();
        assert_eq!(accepted.seq, 1);
        assert_eq!(
            service
                .make_move(&id(), movement("first", 0, "outsider", "7g7f"))
                .await,
            Err(Error::NonParticipant)
        );
    }
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/grpc-command", relationship: crate::trace_support::Relationship::Unreviewed, scope: "製品の公開 API を呼ぶ結合テスト。個々の assertion と契約の対応は未レビュー。", reason: "宣言した実提供元の境界を利用する。契約の各条件を直接検証するかは未レビュー。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-application"], fake: &[] }] };
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
}
