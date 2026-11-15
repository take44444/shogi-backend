crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real worker, PostgreSQL and read adapter for independent pools.",
        reason: "Two real worker ticks produce independent black ratings of 1516 and 1484 in the two pools, one completed game each, two ledgers and no remaining slots.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "rating-application", contract: "worker-reconciliation", scope: "Real worker, PostgreSQL and read adapter for independent pools.", relationship: crate::trace_support::Relationship::Asserts, reason: "Real worker application uses independent frozen starts for the two pools." }, crate::trace_support::Evidence { feature: "user-ratings", contract: "postgres-snapshot", scope: "実 PostgreSQL と PostgresGameRecords。両プールのワーカー適用後の読み取り。HTTP は通らない。", relationship: crate::trace_support::Relationship::Asserts, reason: "実 DB から同じユーザーの両プールに異なる確定値 1516 と 1484、対局数 1 を確認する。" }],
    };
#[tokio::test]
async fn both_pools_update_independently_from_their_frozen_start_states() {
    let f = Fixture::new().await;
    f.create("main-time", Pool::TenMinute30).await;
    f.finish("main-time").await;
    f.create("byoyomi", Pool::Byoyomi30).await;
    GameService::new(f.store.clone())
        .resign(
            &game("byoyomi"),
            Resign {
                request_id: RequestId::new("finish").unwrap(),
                expected_seq: 0,
                user_id: user("black"),
            },
        )
        .await
        .unwrap();
    let worker = RatingWorker::new(f.jobs.clone(), policy(30000, 3));
    assert_eq!(
        worker.tick(&owner("both")).await.unwrap(),
        TickOutcome::Applied
    );
    assert_eq!(
        worker.tick(&owner("both")).await.unwrap(),
        TickOutcome::Applied
    );
    let reader = f.reader().await;
    let snapshot = reader.ratings(&user("black")).await.unwrap();
    assert_eq!(snapshot.ten_minute_30.rating(), Some(1516));
    assert_eq!(snapshot.byoyomi_30.rating(), Some(1484));
    assert_eq!(snapshot.ten_minute_30.completed(), 1);
    assert_eq!(snapshot.byoyomi_30.completed(), 1);
    assert_eq!(f.count("rating_applications").await, 2);
    assert_eq!(f.count("rating_slots").await, 0);
}
}
