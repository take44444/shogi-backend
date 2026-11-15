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
