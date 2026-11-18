use super::*;

#[traceability::targets(game_application::GameService::resign)]
#[tokio::test]
async fn terminal_receipt_outbox_and_rating_job_commit_or_rollback_together() {
    let f = Fixture::new().await;
    f.create("rated", Pool::TenMinute30).await;
    f.raw.execute_unprepared("CREATE FUNCTION reject_rating_job() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected job failure'; END $$; CREATE TRIGGER reject_rating_job BEFORE INSERT ON rating_jobs FOR EACH ROW EXECUTE FUNCTION reject_rating_job()").await.unwrap();
    let games = GameService::new(f.store.clone());
    let command = Resign {
        request_id: RequestId::new("finish").unwrap(),
        expected_seq: 0,
        user_id: user("white"),
    };
    assert_eq!(
        games.resign(&game("rated"), command.clone()).await,
        Err(ApplicationError::Unavailable)
    );
    assert_eq!(f.store.version(&game("rated")).await.unwrap(), 0);
    for table in ["journal", "outbox_events", "rating_jobs"] {
        assert_eq!(f.count(table).await, 0)
    }
    f.raw
        .execute_unprepared("DROP TRIGGER reject_rating_job ON rating_jobs")
        .await
        .unwrap();
    let accepted = games.resign(&game("rated"), command.clone()).await.unwrap();
    assert_eq!(
        games.resign(&game("rated"), command).await.unwrap(),
        accepted
    );
    for table in ["journal", "outbox_events", "rating_jobs"] {
        assert_eq!(f.count(table).await, 1)
    }
    pg_test_support::GameFixture::new(f.store.clone(), f.database.url())
        .seed(game("casual"), players("black", "white"))
        .await
        .unwrap();
    f.finish("casual").await;
    assert_eq!(f.count("rating_jobs").await, 1);
    assert!(
        pg_test_support::rated_identity(f.database.url(), &game("casual"))
            .await
            .is_none()
    );
}
