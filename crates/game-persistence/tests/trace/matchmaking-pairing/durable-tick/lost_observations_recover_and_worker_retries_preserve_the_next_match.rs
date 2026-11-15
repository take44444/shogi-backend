#[tokio::test]
async fn lost_observations_recover_and_worker_retries_preserve_the_next_match() {
    use game_application::rating::{ApplyOutcome, RatingJobRepository, WorkerId, WorkerPolicy};
    use game_persistence::SeaOrmRatingJobs;
    use std::{num::NonZeroU32, time::Duration};
    let f = Fixture::new().await;
    let one = TicketId::new();
    let two = TicketId::new();
    let _ = f.store.join(uid("one"), one, POOL).await.unwrap();
    let recovered = SeaOrmStore::connect(f._db.url()).await.unwrap();
    assert!(!recovered.join(uid("one"), one, POOL).await.unwrap().created);
    f.store.join(uid("two"), two, POOL).await.unwrap();
    let _ = f.store.tick(POOL).await.unwrap();
    let game = match recovered.ticket(&uid("one"), one).await.unwrap().state {
        TicketState::Playing { game_id, .. } => game_id,
        other => panic!("{other:?}"),
    };
    let original = f.store.load_game(&game).await.unwrap();
    GameService::new(f.store.clone())
        .resign(
            &game,
            Resign {
                request_id: RequestId::new("finish").unwrap(),
                user_id: original.participants.white().clone(),
                expected_seq: 0,
            },
        )
        .await
        .unwrap();
    let jobs = SeaOrmRatingJobs::connect(f._db.url()).await.unwrap();
    let policy = WorkerPolicy::new(
        Duration::from_secs(30),
        NonZeroU32::new(3).unwrap(),
        Duration::ZERO,
    )
    .unwrap();
    let claim = jobs
        .claim(&WorkerId::new("worker").unwrap(), &policy)
        .await
        .unwrap()
        .unwrap();
    let input = claim.input().unwrap();
    let change =
        rating_domain::calculate(input.game.black_start, input.game.white_start, input.winner)
            .unwrap();
    assert_eq!(
        jobs.apply(&claim, &change).await.unwrap(),
        ApplyOutcome::Applied
    );
    f.join("one", POOL).await;
    f.join("two", POOL).await;
    let next = f.store.tick(POOL).await.unwrap().unwrap();
    let frozen = pg_test_support::rated_identity(f._db.url(), &next)
        .await
        .unwrap();
    for user in [original.participants.black(), original.participants.white()] {
        let expected = if user == original.participants.black() {
            change.black
        } else {
            change.white
        };
        let actual = if user == frozen.participants.black() {
            frozen.black_start
        } else {
            frozen.white_start
        };
        assert_eq!(actual, expected);
    }
    assert_eq!(
        jobs.apply(&claim, &change).await.unwrap(),
        ApplyOutcome::AlreadyApplied
    );
    assert_eq!(f.count("rating_slots").await, 2);
    assert!(
        matches!(f.store.current(&uid("one")).await.unwrap(),Participation::Active(t) if matches!(t.state,TicketState::Playing {ref game_id,..} if game_id==&next))
    );
}
