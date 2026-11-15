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
