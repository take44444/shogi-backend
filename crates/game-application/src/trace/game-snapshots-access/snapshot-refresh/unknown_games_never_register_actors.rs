#[tokio::test]
async fn unknown_games_never_register_actors() {
    let service = GameService::new(store());
    for index in 0..100 {
        let missing = GameId::new(format!("missing-{index}")).unwrap();
        assert_eq!(
            service.snapshot(&missing).await,
            Err(ApplicationError::NotFound)
        );
        let command = MakeMove {
            request_id: receipt().request_id,
            user_id: receipt().user_id,
            expected_seq: 0,
            movement: "7g7f".parse().unwrap(),
        };
        assert_eq!(
            service.make_move(&missing, command).await,
            Err(ApplicationError::NotFound)
        );
    }
    assert!(service.games.lock().unwrap().is_empty());
}
