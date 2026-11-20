use super::*;

#[traceability::targets(game_persistence::SeaOrmRatingJobs::apply)]
#[tokio::test]
async fn completed_replay_is_read_only_and_never_releases_the_next_games_slots() {
    let f = Fixture::new().await;
    f.create("one", Pool::TenMinute30).await;
    f.finish("one").await;
    let policy = policy(30000, 3);
    let claim = f.jobs.claim(&owner("one"), &policy).await.unwrap().unwrap();
    let change = change(&claim);
    assert_eq!(
        f.jobs.apply(&claim, &change).await.unwrap(),
        ApplyOutcome::Applied
    );
    f.create("two", Pool::TenMinute30).await;
    let next = pg_test_support::rated_identity(f.database.url(), &game("two"))
        .await
        .unwrap();
    assert_eq!(next.black_start.rating(), Some(1516));
    assert_eq!(next.white_start.rating(), Some(1484));
    assert_eq!(
        f.jobs.apply(&claim, &change).await.unwrap(),
        ApplyOutcome::AlreadyApplied
    );
    assert_eq!(
        f.jobs
            .fail(&claim, JobFailure::Transient, &policy)
            .await
            .unwrap(),
        TickOutcome::LostClaim
    );
    assert_eq!(f.count("rating_slots").await, 2);
    assert_eq!(f.count("rating_applications").await, 1);
    let reader = f.reader().await;
    let snapshot = reader.ratings(&user("black")).await.unwrap();
    assert_eq!(snapshot.ten_minute_30.rating(), Some(1516));
    assert_eq!(snapshot.ten_minute_30.completed(), 1);
    assert_eq!(snapshot.byoyomi_30, RatingState::Unrated);
    assert_eq!(
        reader
            .ratings(&user("unknown"))
            .await
            .unwrap()
            .ten_minute_30,
        RatingState::Unrated
    );
    assert_eq!(
        pg_test_support::seed_rated(
            f.database.url(),
            &game("blocked"),
            &players("black", "third"),
            Pool::TenMinute30
        )
        .await,
        Err(ApplicationError::RatingPoolBusy)
    );
}

#[traceability::targets(game_application::rating::RatingWorker::tick)]
#[tokio::test]
async fn both_pools_update_independently_from_their_frozen_start_states() {
    let f = Fixture::new().await;
    f.create("main-time", Pool::TenMinute30).await;
    f.finish("main-time").await;
    f.create("byoyomi", Pool::Byoyomi30).await;
    GameService::new(f.store.clone())
        .resign(
            &game("byoyomi"),
            Resign {
                request_id: RequestId::new("finish").unwrap(),
                expected_seq: 0,
                user_id: user("black"),
            },
        )
        .await
        .unwrap();
    let worker = RatingWorker::new(f.jobs.clone(), policy(30000, 3));
    assert_eq!(
        worker.tick(&owner("both")).await.unwrap(),
        TickOutcome::Applied
    );
    assert_eq!(
        worker.tick(&owner("both")).await.unwrap(),
        TickOutcome::Applied
    );
    let reader = f.reader().await;
    let snapshot = reader.ratings(&user("black")).await.unwrap();
    assert_eq!(snapshot.ten_minute_30.rating(), Some(1516));
    assert_eq!(snapshot.byoyomi_30.rating(), Some(1484));
    assert_eq!(snapshot.ten_minute_30.completed(), 1);
    assert_eq!(snapshot.byoyomi_30.completed(), 1);
    assert_eq!(f.count("rating_applications").await, 2);
    assert_eq!(f.count("rating_slots").await, 0);
}

#[traceability::targets(game_persistence::SeaOrmRatingJobs::apply)]
#[tokio::test]
async fn failed_ledger_insert_rolls_back_both_ratings_and_reservations() {
    let f = Fixture::new().await;
    f.create("atomic", Pool::TenMinute30).await;
    f.finish("atomic").await;
    let claim = f
        .jobs
        .claim(&owner("atomic"), &policy(30000, 3))
        .await
        .unwrap()
        .unwrap();
    f.raw.execute_unprepared("CREATE FUNCTION reject_application() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected ledger failure'; END $$; CREATE TRIGGER reject_application BEFORE INSERT ON rating_applications FOR EACH ROW EXECUTE FUNCTION reject_application()").await.unwrap();
    assert_eq!(
        f.jobs.apply(&claim, &change(&claim)).await,
        Err(RatingWorkerError::Unavailable)
    );
    assert_eq!(f.count("rating_applications").await, 0);
    assert_eq!(f.count("rating_slots").await, 2);
    let reader = f.reader().await;
    for who in ["black", "white"] {
        assert_eq!(
            reader.ratings(&user(who)).await.unwrap().ten_minute_30,
            RatingState::Unrated
        )
    }
    f.raw
        .execute_unprepared("DROP TRIGGER reject_application ON rating_applications")
        .await
        .unwrap();
    assert_eq!(
        f.jobs.apply(&claim, &change(&claim)).await.unwrap(),
        ApplyOutcome::Applied
    );
}
