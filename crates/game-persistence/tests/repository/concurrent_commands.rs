use super::*;

#[traceability::targets(game_application::GameService::make_move)]
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
    assert_same_snapshot(
        a.snapshot(&game).await.unwrap(),
        &b.snapshot(&game).await.unwrap(),
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

#[traceability::targets(game_application::GameService::make_move)]
#[tokio::test]
async fn uncertain_outcomes_reconcile_durable_receipts_or_fence_until_recovery() {
    for mode in [1, 2, 3, 4] {
        let database = TestDatabase::new().await;
        let (store, _) = setup(database.url()).await;
        let fault = Arc::new(FaultStore {
            inner: store.clone(),
            mode: AtomicU8::new(mode),
            unreadable: Arc::new(AtomicBool::new(false)),
        });
        let service = pg_test_support::GameFixture::new(fault.clone(), database.url());
        let command = movement("request", 0, "black", "7g7f");
        let response = service.make_move(&id(), command.clone()).await;
        match mode {
            2 => assert_eq!(response.unwrap().seq, 1),
            4 => assert_eq!(response, Err(Error::RequestConflict)),
            _ => assert_eq!(response, Err(Error::Unavailable)),
        }
        if mode == 3 {
            assert_eq!(service.snapshot(&id()).await, Err(Error::Unavailable));
            assert_eq!(
                service
                    .resign(
                        &id(),
                        Resign {
                            request_id: RequestId::new("next").unwrap(),
                            expected_seq: 1,
                            user_id: user("white")
                        }
                    )
                    .await,
                Err(Error::Unavailable)
            );
            fault.unreadable.store(false, Ordering::SeqCst);
        }
        let record = store.load_game(&id()).await.unwrap();
        assert_eq!(record.journal.len(), usize::from(mode != 1));
        assert_eq!(
            service.snapshot(&id()).await.unwrap().seq,
            u64::from(mode != 1)
        );
        if mode != 4 {
            assert_eq!(service.make_move(&id(), command).await.unwrap().seq, 1);
        }
        assert_eq!(store.load_game(&id()).await.unwrap().journal.len(), 1);
    }
}

#[traceability::targets(game_application::GameService::poll_timeouts)]
#[tokio::test]
async fn uncertain_timeout_commit_reconciles_system_history_without_a_user_receipt() {
    let database = TestDatabase::new().await;
    let (store, _) = setup(database.url()).await;
    let raw = Database::connect(database.url()).await.unwrap();
    raw.execute_unprepared("UPDATE games SET created_at=created_at-631000,clock_started=clock_started-631000,clock_deadline=clock_deadline-631000").await.unwrap();
    let fault = Arc::new(FaultStore {
        inner: store.clone(),
        mode: AtomicU8::new(2),
        unreadable: Arc::new(AtomicBool::new(false)),
    });
    let service = game_application::GameService::new(fault);
    assert_eq!(service.poll_timeouts(16).await.unwrap(), 1);
    assert_eq!(service.poll_timeouts(16).await.unwrap(), 0);
    let record = store.load_game(&id()).await.unwrap();
    assert_eq!(record.version, 1);
    assert!(matches!(
        record.journal[0],
        game_application::JournalEntry::Timeout { .. }
    ));
    assert_eq!(
        record.result.unwrap().reason,
        shogi_domain::FinishReason::Timeout
    );
    assert_eq!(
        integer_query(&raw, "SELECT count(*) AS value FROM outbox_events").await,
        1
    );
    assert_eq!(service.snapshot(&id()).await.unwrap().clock, record.clock);
    raw.close().await.unwrap();
    store.close().await.unwrap();
}
