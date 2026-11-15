crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport. Separate real store/service instances.",
        reason: "Independent real store/service instances serialize CAS conflicts and identical concurrent retries.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "game-command-recovery", contract: "durable-commit", scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport. Separate real store/service instances.", relationship: crate::trace_support::Relationship::Asserts, reason: "Independent service/store instances CAS and refresh correctly." }],
    };
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn independent_instances_cas_refresh_and_identical_concurrent_replay() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let (store, a) = setup(&url).await;
    let b = pg_test_support::GameFixture::new(
        Arc::new(SeaOrmStore::connect(&url).await.unwrap()),
        &url,
    );
    assert_eq!(b.snapshot(&id()).await.unwrap().seq, 0);
    let game = id();
    let (left, right) = tokio::join!(
        a.make_move(&game, movement("a", 0, "black", "7g7f")),
        b.make_move(&game, movement("b", 0, "black", "2g2f"))
    );
    assert!(matches!(
        (left, right),
        (Ok(_), Err(Error::SequenceMismatch { actual: 1, .. }))
            | (Err(Error::SequenceMismatch { actual: 1, .. }), Ok(_))
    ));
    assert_eq!(
        a.snapshot(&game).await.unwrap(),
        b.snapshot(&game).await.unwrap()
    );
    let command = movement("white", 1, "white", "3c3d");
    let (left, right) = tokio::join!(
        a.make_move(&game, command.clone()),
        b.make_move(&game, command)
    );
    assert_eq!(left.unwrap(), right.unwrap());
    assert_eq!(store.load_game(&game).await.unwrap().journal.len(), 2);
    store.close().await.unwrap();
    assert_eq!(a.snapshot(&game).await, Err(Error::Unavailable));
    assert_eq!(
        a.make_move(&game, movement("white", 1, "white", "3c3d"))
            .await,
        Err(Error::Unavailable)
    );
}
}
