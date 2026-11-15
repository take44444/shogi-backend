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
