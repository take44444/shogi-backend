#[tokio::test]
async fn admission_waits_for_application_and_freezes_the_new_committed_states() {
    let f = Fixture::new().await;
    f.create("previous", Pool::TenMinute30).await;
    f.finish("previous").await;
    let claim = f
        .jobs
        .claim(&owner("applying"), &policy(30000, 3))
        .await
        .unwrap()
        .unwrap();
    let delta = change(&claim);
    let held = f.raw.begin().await.unwrap();
    held.execute_unprepared("SELECT * FROM rating_states FOR UPDATE")
        .await
        .unwrap();
    let jobs = f.jobs.clone();
    let applying = tokio::spawn(async move { jobs.apply(&claim, &delta).await });
    wait_for_blocked_transactions(&f, 1).await;
    let store = f.store.clone();
    let admission = tokio::spawn(async move {
        store
            .join(user("black"), TicketId::new(), Pool::TenMinute30)
            .await
    });
    wait_for_blocked_transactions(&f, 2).await;
    held.commit().await.unwrap();
    assert_eq!(applying.await.unwrap().unwrap(), ApplyOutcome::Applied);
    assert!(admission.await.unwrap().unwrap().created);
    f.store
        .join(user("white"), TicketId::new(), Pool::TenMinute30)
        .await
        .unwrap();
    let next = f.store.tick(Pool::TenMinute30).await.unwrap().unwrap();
    let identity = pg_test_support::rated_identity(f.database.url(), &next)
        .await
        .unwrap();
    let (black, white) = if identity.participants.black() == &user("black") {
        (identity.black_start, identity.white_start)
    } else {
        (identity.white_start, identity.black_start)
    };
    assert_eq!(black.rating(), Some(1516));
    assert_eq!(white.rating(), Some(1484));
    assert_eq!(black.completed(), 1);
    assert_eq!(white.completed(), 1);
    assert_eq!(f.count("rating_slots").await, 2);
}
