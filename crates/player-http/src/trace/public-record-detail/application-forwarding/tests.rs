use super::*;
use crate::record_route_support::unused_ratings;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real Axum router, application service and PostgreSQL reader; asserts public serialization and missing/ongoing behavior.",
        reason: "The real router traverses this method into the PostgreSQL reader.",
        relationship: crate::trace_support::Relationship::Exercises,
        also: &[crate::trace_support::Evidence { feature: "public-record-detail", contract: "http-boundary", scope: "Real Axum router, application service and PostgreSQL reader; asserts public serialization and missing/ongoing behavior.", relationship: crate::trace_support::Relationship::Asserts, reason: "Missing and ongoing real database records yield identical 404 status/body, and successful detail has no-store." }, crate::trace_support::Evidence { feature: "public-record-detail", contract: "public-dto", scope: "Real Axum router, application service and PostgreSQL reader; asserts public serialization and missing/ongoing behavior.", relationship: crate::trace_support::Relationship::Asserts, reason: "The exact metadata key set, equality with list metadata and two ordered operations are asserted; serialized detail excludes each listed secret substring." }, crate::trace_support::Evidence { feature: "public-record-search", contract: "query-boundary", scope: "Real Axum router, application service and PostgreSQL reader; no player TCP listener.", relationship: crate::trace_support::Relationship::Asserts, reason: "A real router and PostgreSQL reader return the one record matching a combined participant/rule/winner/reason/ply query." }, crate::trace_support::Evidence { feature: "public-record-search", contract: "application-forwarding", scope: "Real Axum router, application service and PostgreSQL reader; no player TCP listener.", relationship: crate::trace_support::Relationship::Exercises, reason: "The real router traverses this application method to a real PostgreSQL reader." }, crate::trace_support::Evidence { feature: "public-record-search", contract: "public-keysets", scope: "Real Axum router, application service and PostgreSQL reader; no player TCP listener.", relationship: crate::trace_support::Relationship::Asserts, reason: "The anonymous real router returns only public metadata and becomes unavailable after closing the reader." }],
    };
#[tokio::test]
async fn anonymous_list_is_metadata_only_and_detail_contains_only_public_ordered_actions() {
    let database = pg_test_support::TestDatabase::new().await;
    let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
    let games = pg_test_support::GameFixture::new(store, database.url());
    let id = WriteId::new("game").unwrap();
    games
        .seed(
            id.clone(),
            Participants::new(UserId::new("black").unwrap(), UserId::new("white").unwrap())
                .unwrap(),
        )
        .await
        .unwrap();
    games
        .make_move(
            &id,
            MakeMove {
                request_id: RequestId::new("private-retry-id").unwrap(),
                expected_seq: 0,
                user_id: UserId::new("black").unwrap(),
                movement: "7g7f".parse().unwrap(),
            },
        )
        .await
        .unwrap();
    games
        .resign(
            &id,
            Resign {
                request_id: RequestId::new("private-finish-id").unwrap(),
                expected_seq: 1,
                user_id: UserId::new("white").unwrap(),
            },
        )
        .await
        .unwrap();
    games
        .seed(
            WriteId::new("ongoing").unwrap(),
            Participants::new(UserId::new("black").unwrap(), UserId::new("white").unwrap())
                .unwrap(),
        )
        .await
        .unwrap();
    let reader = Arc::new(
        PostgresGameRecords::connect(&database.reader_url().await)
            .await
            .unwrap(),
    );
    let app = router(
        GameRecords::new(reader.clone()),
        UserRatings::new(reader.clone()),
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as i64;
    let query = format!(
        "/game-records?from={}&to={}&participant=white&rule=standard&winner=black&reason=resignation&min_ply=1&max_ply=1&limit=1",
        now - 1_000_000,
        now + 1_000_000
    );
    let (status, list) = get(app.clone(), &query).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    let item = &list["items"][0];
    assert_eq!(item["id"], "game");
    assert_eq!(item["final_seq"], 2);
    assert_eq!(item["final_move_ply"], 1);
    let mut keys = item
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "black",
            "final_move_ply",
            "final_seq",
            "finished_at",
            "id",
            "result",
            "rule",
            "white"
        ]
    );
    let (status, detail) = get(app.clone(), "/game-records/game").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["metadata"], *item);
    assert_eq!(
        detail["history"],
        serde_json::json!([
            {"seq":1,"player":"black","action":"move","usi":"7g7f"},
            {"seq":2,"player":"white","action":"resign"}
        ])
    );
    let encoded = serde_json::to_string(&detail).unwrap();
    for secret in [
        "request_id",
        "expected_seq",
        "acceptance",
        "receipt",
        "in_check",
        "private-retry-id",
        "private-finish-id",
        "outbox",
    ] {
        assert!(!encoded.contains(secret));
    }
    let missing = get(app.clone(), "/game-records/missing").await;
    let ongoing = get(app.clone(), "/game-records/ongoing").await;
    assert_eq!(
        missing,
        (
            StatusCode::NOT_FOUND,
            serde_json::json!({"error":"not_found"})
        )
    );
    assert_eq!(ongoing, missing);
    reader.close().await;
    assert_eq!(
        get(app, &query).await,
        (
            StatusCode::SERVICE_UNAVAILABLE,
            serde_json::json!({"error":"unavailable"})
        )
    );
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "In-process router with fake repository failures; no database.",
        reason: "Fake repository errors traverse this method before HTTP mapping.",
        relationship: crate::trace_support::Relationship::Exercises,
        also: &[crate::trace_support::Evidence { feature: "public-record-detail", contract: "http-boundary", scope: "In-process router with fake repository failures; no database.", relationship: crate::trace_support::Relationship::Asserts, reason: "Fake corrupt, oversized and unavailable failures become 500/422/503 and fixed JSON with no-store." }],
    };
#[tokio::test]
async fn bounded_detail_and_storage_errors_are_generic() {
    for (failure, status, code) in [
        (
            Error::Corrupt,
            StatusCode::INTERNAL_SERVER_ERROR,
            "invalid_record",
        ),
        (
            Error::HistoryTooLong,
            StatusCode::UNPROCESSABLE_ENTITY,
            "history_too_long",
        ),
        (
            Error::Unavailable,
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
        ),
    ] {
        let app = router(
            GameRecords::new(Arc::new(Failing(failure))),
            unused_ratings(),
        );
        assert_eq!(
            get(app, "/game-records/game").await,
            (status, serde_json::json!({"error":code}))
        );
    }
}
}
