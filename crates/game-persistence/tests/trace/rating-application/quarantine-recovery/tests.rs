use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の RatingWorker と PostgreSQL で、破損、隔離、明示的な復旧、修復を検証する。",
        reason: "RatingWorker による復旧は、破損した現在状態を修復しない。後の tick を成功させるには、状態を修復する必要がある。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "rating-application", contract: "worker-reconciliation", scope: "実際の RatingWorker と PostgreSQL で、破損、隔離、明示的な復旧、修復を検証する。", relationship: crate::trace_support::Relationship::Asserts, reason: "実際のワーカーは破損した現在状態を隔離し、明示的な再試行を成功させるには、その状態の修復も必要となる。" }],
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
        scope: "実際の PostgreSQL で、一時的な失敗後の待機時間と試行回数の上限を検証する。",
        reason: "一時的な失敗は待機時間の終了前に再取得できず、設定した試行上限に達すると隔離される。",
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
