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
