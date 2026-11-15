crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "In-process Axum admission using a deliberately blocked fake repository.",
        reason: "Sixteen blocked searches enter storage, the seventeenth gets immediate 503 and no-store, and capacity returns after release.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn sixteen_active_requests_admit_no_seventeenth_and_release_capacity() {
    let (entered, mut arrivals) = mpsc::unbounded_channel();
    let repository = Arc::new(Blocked {
        entered,
        release: Semaphore::new(0),
    });
    let app = router(GameRecords::new(repository.clone()), unused_ratings());
    let mut requests = Vec::new();
    for _ in 0..16 {
        let app = app.clone();
        requests.push(tokio::spawn(async move {
            request(app, "/game-records?from=0&to=31622400000000").await
        }));
    }
    for _ in 0..16 {
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(5), arrivals.recv())
                .await
                .unwrap(),
            Some(30)
        );
    }
    let rejected = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        request(app.clone(), "/game-records?from=0&to=100"),
    )
    .await
    .expect("the seventeenth request must be rejected without waiting for storage");
    assert_eq!(rejected.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(rejected.headers()[header::CACHE_CONTROL], "no-store");
    assert!(arrivals.try_recv().is_err());
    repository.release.add_permits(16);
    for task in requests {
        assert_eq!(task.await.unwrap().status(), StatusCode::OK);
    }
    let last =
        tokio::spawn(async move { request(app, "/game-records?from=0&to=100&limit=100").await });
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(5), arrivals.recv())
            .await
            .unwrap(),
        Some(100)
    );
    repository.release.add_permits(1);
    assert_eq!(last.await.unwrap().status(), StatusCode::OK);
}
}
