crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport. FaultStore wraps real PostgreSQL to control return/read failures.",
        reason: "Commit-after-error reconciles receipt; precommit failure fences until durable reads recover.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "game-command-recovery", contract: "reconciliation", scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport. FaultStore wraps real PostgreSQL to control return/read failures.", relationship: crate::trace_support::Relationship::Asserts, reason: "Fault adapter around real PostgreSQL tests commit-after-error receipt recovery, precommit failure, unreadable fencing and later recovery." }],
    };
#[tokio::test]
async fn uncertain_outcomes_reconcile_durable_receipts_or_fence_until_recovery() {
    for mode in [1, 2, 3, 4] {
        let database = TestDatabase::new().await;
        let (store, _) = setup(database.url()).await;
        let fault = Arc::new(FaultStore {
            inner: store.clone(),
            mode: AtomicU8::new(mode),
            unreadable: AtomicBool::new(false),
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
}
