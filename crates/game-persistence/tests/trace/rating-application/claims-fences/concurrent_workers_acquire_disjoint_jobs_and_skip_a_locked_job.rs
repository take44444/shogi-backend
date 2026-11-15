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
