crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL journal/metadata corruption and oversized-history rejection.",
        reason: "Eight corruption mutations and excess history are rejected rather than returning partial detail.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn corrupt_and_oversized_details_fail_without_partial_history() {
    for statement in [
        "UPDATE games SET final_move_ply=0",
        "UPDATE games SET version=3,final_move_ply=2",
        "UPDATE journal SET user_id='stranger' WHERE seq=1",
        "UPDATE journal SET usi='not-usi' WHERE seq=1",
        "UPDATE journal SET winner=2,reason=2 WHERE seq=1",
        "UPDATE journal SET winner=2,reason=2 WHERE seq=2",
        "UPDATE journal SET user_id='black' WHERE seq=2",
        "DELETE FROM outbox_events; DELETE FROM journal WHERE seq=1",
    ] {
        let f = Fixture::new().await;
        f.completed("game", "black", "white", 1, "white", 10).await;
        f.raw.execute_unprepared(statement).await.unwrap();
        assert_eq!(
            f.reader().await.detail(&GameId::new("game").unwrap()).await,
            Err(Error::Corrupt),
            "{statement}"
        );
    }
    let f = Fixture::new().await;
    f.completed("game", "black", "white", 0, "white", 10).await;
    f.raw
        .execute_unprepared(&format!(
            "UPDATE games SET version={},final_move_ply={}",
            MAX_HISTORY + 1,
            MAX_HISTORY
        ))
        .await
        .unwrap();
    assert_eq!(
        f.reader().await.detail(&GameId::new("game").unwrap()).await,
        Err(Error::HistoryTooLong)
    );
}
}
