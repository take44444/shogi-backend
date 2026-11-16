use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real RatingWorker/PostgreSQL corruption, quarantine, explicit recovery and repair.",
        reason: "Recovery through RatingWorker does not repair corrupt current state; repaired state is required for a later successful tick.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "rating-application", contract: "worker-reconciliation", scope: "Real RatingWorker/PostgreSQL corruption, quarantine, explicit recovery and repair.", relationship: crate::trace_support::Relationship::Asserts, reason: "A real worker quarantines corrupt current state and successful explicit retry still requires repairing that state." }],
    };
#[tokio::test]
async fn corrupt_current_state_quarantines_and_retry_requires_repair() {
    let f = Fixture::new().await;
    f.create("corrupt", Pool::TenMinute30).await;
    f.finish("corrupt").await;
    f.raw
        .execute_unprepared(
            "UPDATE rating_states SET rating=1600,completed=1 WHERE user_id='black'",
        )
        .await
        .unwrap();
    let worker = RatingWorker::new(f.jobs.clone(), policy(30000, 3));
    assert_eq!(
        worker.tick(&owner("worker")).await.unwrap(),
        TickOutcome::Quarantined
    );
    assert_eq!(f.count("rating_applications").await, 0);
    assert_eq!(f.count("rating_slots").await, 2);
    assert_eq!(
        worker.retry_quarantined(&game("corrupt")).await.unwrap(),
        RecoveryOutcome::Retried
    );
    assert_eq!(
        worker.tick(&owner("worker")).await.unwrap(),
        TickOutcome::Quarantined
    );
    f.raw
        .execute_unprepared(
            "UPDATE rating_states SET rating=NULL,completed=0 WHERE user_id='black'",
        )
        .await
        .unwrap();
    worker.retry_quarantined(&game("corrupt")).await.unwrap();
    assert_eq!(
        worker.tick(&owner("worker")).await.unwrap(),
        TickOutcome::Applied
    );
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL transient backoff and attempt-limit behavior.",
        reason: "Transient failure cannot be reacquired before backoff and reaches quarantine at the configured attempt limit.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn transient_failure_obeys_backoff_and_quarantines_at_the_attempt_limit() {
    let f = Fixture::new().await;
    f.create("retry", Pool::TenMinute30).await;
    f.finish("retry").await;
    let policy = WorkerPolicy::new(
        Duration::from_secs(30),
        NonZeroU32::new(2).unwrap(),
        Duration::from_secs(30),
    )
    .unwrap();
    let first = f.jobs.claim(&owner("one"), &policy).await.unwrap().unwrap();
    assert_eq!(
        f.jobs
            .fail(&first, JobFailure::Transient, &policy)
            .await
            .unwrap(),
        TickOutcome::Deferred
    );
    assert!(
        f.jobs
            .claim(&owner("early"), &policy)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        f.jobs
            .fail(&first, JobFailure::Corrupt, &policy)
            .await
            .unwrap(),
        TickOutcome::LostClaim
    );
    f.raw
        .execute_unprepared("UPDATE rating_jobs SET available_at=0")
        .await
        .unwrap();
    let second = f.jobs.claim(&owner("two"), &policy).await.unwrap().unwrap();
    assert_eq!(
        f.jobs
            .fail(&second, JobFailure::Transient, &policy)
            .await
            .unwrap(),
        TickOutcome::Quarantined
    );
    assert!(
        f.jobs
            .claim(&owner("exhausted"), &policy)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(f.count("rating_slots").await, 2);
    assert_eq!(f.count("rating_applications").await, 0);
}
}
