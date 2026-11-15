crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL claims, expired/stale fences, budget and recovery; worker error routing is not the subject.",
        reason: "Generations increase, expired/stale apply and fail return LostClaim, acquisition budget exhausts, slots remain reserved and recovery outcomes are checked.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "rating-application", contract: "quarantine-recovery", scope: "Real PostgreSQL claims, expired/stale fences, budget and recovery; worker error routing is not the subject.", relationship: crate::trace_support::Relationship::Asserts, reason: "Expired-claim budget exhaustion retains slots; recovery and repeated/unknown recovery outcomes are directly checked." }],
    };
#[tokio::test]
async fn expired_claims_consume_budget_quarantine_and_recover_without_releasing_slots() {
    let f = Fixture::new().await;
    f.create("expiry", Pool::TenMinute30).await;
    f.finish("expiry").await;
    let policy = policy(30000, 2);
    let old = f.jobs.claim(&owner("old"), &policy).await.unwrap().unwrap();
    f.raw
        .execute_unprepared("UPDATE rating_jobs SET lease_until=0")
        .await
        .unwrap();
    assert_eq!(
        f.jobs
            .fail(&old, JobFailure::Transient, &policy)
            .await
            .unwrap(),
        TickOutcome::LostClaim
    );
    let new = f.jobs.claim(&owner("new"), &policy).await.unwrap().unwrap();
    assert!(new.generation() > old.generation());
    assert_eq!(
        f.jobs.apply(&old, &change(&old)).await.unwrap(),
        ApplyOutcome::LostClaim
    );
    assert_eq!(
        f.jobs
            .fail(&old, JobFailure::Corrupt, &policy)
            .await
            .unwrap(),
        TickOutcome::LostClaim
    );
    f.raw
        .execute_unprepared("UPDATE rating_jobs SET lease_until=0")
        .await
        .unwrap();
    assert!(
        f.jobs
            .claim(&owner("third"), &policy)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(f.count("rating_slots").await, 2);
    assert_eq!(f.count("rating_applications").await, 0);
    assert_eq!(
        f.jobs.retry_quarantined(&game("expiry")).await.unwrap(),
        RecoveryOutcome::Retried
    );
    assert_eq!(
        f.jobs.retry_quarantined(&game("expiry")).await.unwrap(),
        RecoveryOutcome::NotQuarantined
    );
    assert_eq!(
        f.jobs.retry_quarantined(&game("missing")).await.unwrap(),
        RecoveryOutcome::NotFound
    );
    assert_eq!(
        f.jobs
            .fail(&new, JobFailure::Transient, &policy)
            .await
            .unwrap(),
        TickOutcome::LostClaim
    );
    let recovered = f
        .jobs
        .claim(&owner("recovered"), &policy)
        .await
        .unwrap()
        .unwrap();
    assert!(recovered.generation() > new.generation());
    assert_eq!(
        f.jobs.apply(&recovered, &change(&recovered)).await.unwrap(),
        ApplyOutcome::Applied
    );
    assert_eq!(f.count("rating_slots").await, 0);
}
}
