use super::*;

#[traceability::targets(game_application::GameService::resign)]
#[tokio::test]
async fn missing_both_terminal_tickets_rolls_back_result_receipt_events_and_job() {
    let f = Fixture::new().await;
    f.join("one", POOL).await;
    f.join("two", POOL).await;
    let game = f.store.tick(POOL).await.unwrap().unwrap();
    f.ready("one").await;
    f.ready("two").await;
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
