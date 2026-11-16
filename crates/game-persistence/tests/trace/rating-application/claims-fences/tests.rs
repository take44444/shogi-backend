use super::*;

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

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL lock wait crossing lease expiry; no CLI.",
        reason: "Lease expiry while waiting for a state lock yields LostClaim, no ledger, two retained reservations and an unchanged unrated black state.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn lease_expiring_while_waiting_for_state_lock_rolls_back_all_mutations() {
    let f = Fixture::new().await;
    f.create("wait", Pool::TenMinute30).await;
    f.finish("wait").await;
    let claim = f
        .jobs
        .claim(&owner("waiting"), &policy(300, 3))
        .await
        .unwrap()
        .unwrap();
    let delta = change(&claim);
    let held = f.raw.begin().await.unwrap();
    held.execute_unprepared("SELECT * FROM rating_states FOR UPDATE")
        .await
        .unwrap();
    let jobs = f.jobs.clone();
    let task = tokio::spawn(async move { jobs.apply(&claim, &delta).await });
    tokio::time::timeout(Duration::from_secs(3),async {
        loop {
            let n:i64=f.raw.query_one_raw(Statement::from_string(DbBackend::Postgres,"SELECT count(*) AS n FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock'")).await.unwrap().unwrap().try_get("","n").unwrap();
            if n>0 {break}tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.unwrap();
    f.raw
        .execute_unprepared("SELECT pg_sleep(0.35)")
        .await
        .unwrap();
    held.commit().await.unwrap();
    assert_eq!(task.await.unwrap().unwrap(), ApplyOutcome::LostClaim);
    assert_eq!(f.count("rating_applications").await, 0);
    assert_eq!(f.count("rating_slots").await, 2);
    assert_eq!(
        f.reader()
            .await
            .ratings(&user("black"))
            .await
            .unwrap()
            .ten_minute_30,
        RatingState::Unrated
    );
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Concurrent real PostgreSQL acquisitions and a held row lock.",
        reason: "A locked job is skipped; two concurrent claims for the remaining job yield exactly one owner.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn concurrent_workers_acquire_disjoint_jobs_and_skip_a_locked_job() {
    let f = Fixture::new().await;
    f.create("locked", Pool::TenMinute30).await;
    f.finish("locked").await;
    f.create("available", Pool::Byoyomi30).await;
    f.finish("available").await;
    let held = f.raw.begin().await.unwrap();
    held.execute_unprepared("SELECT * FROM rating_jobs WHERE game_id='locked' FOR UPDATE")
        .await
        .unwrap();
    let acquired = tokio::time::timeout(
        Duration::from_secs(2),
        f.jobs.claim(&owner("worker"), &policy(30000, 3)),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    assert_eq!(acquired.game_id(), &game("available"));
    held.commit().await.unwrap();
    let first_owner = owner("one");
    let second_owner = owner("two");
    let claim_policy = policy(30000, 3);
    let (first, second) = tokio::join!(
        f.jobs.claim(&first_owner, &claim_policy),
        f.jobs.claim(&second_owner, &claim_policy)
    );
    let claims = [first.unwrap(), second.unwrap()];
    assert_eq!(claims.iter().filter(|c| c.is_some()).count(), 1);
    assert_eq!(
        claims.into_iter().flatten().next().unwrap().game_id(),
        &game("locked")
    );
}
}
