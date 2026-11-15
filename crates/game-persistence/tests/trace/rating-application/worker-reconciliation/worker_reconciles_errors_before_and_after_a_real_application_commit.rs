crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real RatingWorker and PostgreSQL with unavailable-result injection around actual application commits.",
        reason: "Injected unavailable results before and after real commits produce Applied or AlreadyApplied, then Idle, with one ledger.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn worker_reconciles_errors_before_and_after_a_real_application_commit() {
    for after_commit in [false, true] {
        let f = Fixture::new().await;
        f.create("uncertain", Pool::TenMinute30).await;
        f.finish("uncertain").await;
        let repository = Arc::new(Uncertain {
            inner: f.jobs.clone(),
            first: AtomicBool::new(true),
            after_commit,
        });
        let worker = RatingWorker::new(repository, policy(30000, 3));
        assert_eq!(
            worker.tick(&owner("uncertain")).await.unwrap(),
            if after_commit {
                TickOutcome::AlreadyApplied
            } else {
                TickOutcome::Applied
            }
        );
        assert_eq!(
            worker.tick(&owner("uncertain")).await.unwrap(),
            TickOutcome::Idle
        );
        assert_eq!(f.count("rating_applications").await, 1);
        assert_eq!(f.count("rating_slots").await, 0);
    }
}
}
