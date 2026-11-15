crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL atomic-apply rollback after forced ledger insertion failure.",
        reason: "A forced ledger-insert failure rolls back both ratings and reservations, then succeeds after repair.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn failed_ledger_insert_rolls_back_both_ratings_and_reservations() {
    let f = Fixture::new().await;
    f.create("atomic", Pool::TenMinute30).await;
    f.finish("atomic").await;
    let claim = f
        .jobs
        .claim(&owner("atomic"), &policy(30000, 3))
        .await
        .unwrap()
        .unwrap();
    f.raw.execute_unprepared("CREATE FUNCTION reject_application() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected ledger failure'; END $$; CREATE TRIGGER reject_application BEFORE INSERT ON rating_applications FOR EACH ROW EXECUTE FUNCTION reject_application()").await.unwrap();
    assert_eq!(
        f.jobs.apply(&claim, &change(&claim)).await,
        Err(RatingWorkerError::Unavailable)
    );
    assert_eq!(f.count("rating_applications").await, 0);
    assert_eq!(f.count("rating_slots").await, 2);
    let reader = f.reader().await;
    for who in ["black", "white"] {
        assert_eq!(
            reader.ratings(&user(who)).await.unwrap().ten_minute_30,
            RatingState::Unrated
        )
    }
    f.raw
        .execute_unprepared("DROP TRIGGER reject_application ON rating_applications")
        .await
        .unwrap();
    assert_eq!(
        f.jobs.apply(&claim, &change(&claim)).await.unwrap(),
        ApplyOutcome::Applied
    );
}
}
