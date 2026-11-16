use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL completed replay and later-game reservation preservation.",
        reason: "Completed replay neither adds a ledger nor releases a newer game's slots.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "user-ratings", contract: "postgres-snapshot", scope: "実 PostgreSQL と PostgresGameRecords。ワーカー適用後の読み取り。HTTP は通らない。", relationship: crate::trace_support::Relationship::Asserts, reason: "実 DB から black の確定済み 1516・1、未適用プールの Unrated、unknown の ten_minute_30 の Unrated を確認する。" }],
    };
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
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real worker, PostgreSQL and read adapter for independent pools.",
        reason: "Two real worker ticks produce independent black ratings of 1516 and 1484 in the two pools, one completed game each, two ledgers and no remaining slots.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "rating-application", contract: "worker-reconciliation", scope: "Real worker, PostgreSQL and read adapter for independent pools.", relationship: crate::trace_support::Relationship::Asserts, reason: "Real worker application uses independent frozen starts for the two pools." }, crate::trace_support::Evidence { feature: "user-ratings", contract: "postgres-snapshot", scope: "実 PostgreSQL と PostgresGameRecords。両プールのワーカー適用後の読み取り。HTTP は通らない。", relationship: crate::trace_support::Relationship::Asserts, reason: "実 DB から同じユーザーの両プールに異なる確定値 1516 と 1484、対局数 1 を確認する。" }],
    };
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
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL atomic-apply rollback after forced ledger insertion failure.",
        reason: "A forced ledger-insert failure rolls back both ratings and reservations, then succeeds after repair.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}
