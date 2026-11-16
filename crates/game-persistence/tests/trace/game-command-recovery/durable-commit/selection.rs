use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport. Terminal invariant failure via direct DB fixture changes.",
        reason: "Missing both terminal ticket records prevents result/receipt/outbox/job writes together.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn missing_both_terminal_tickets_rolls_back_result_receipt_events_and_job() {
    let f = Fixture::new().await;
    f.join("one", POOL).await;
    f.join("two", POOL).await;
    let game = f.store.tick(POOL).await.unwrap().unwrap();
    f.raw
        .execute_unprepared("DELETE FROM matchmaking_tickets")
        .await
        .unwrap();
    let result = GameService::new(f.store.clone())
        .resign(
            &game,
            Resign {
                request_id: RequestId::new("finish").unwrap(),
                user_id: uid("one"),
                expected_seq: 0,
            },
        )
        .await;
    assert_eq!(result, Err(game_application::ApplicationError::Unavailable));
    assert_eq!(f.store.version(&game).await.unwrap(), 0);
    for table in ["journal", "outbox_events", "rating_jobs"] {
        assert_eq!(f.count(table).await, 0);
    }
    assert_eq!(f.count("rating_slots").await, 2);
}
}
