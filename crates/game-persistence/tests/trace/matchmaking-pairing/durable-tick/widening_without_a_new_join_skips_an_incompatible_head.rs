crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport. Direct tick after forced enqueue aging.",
        reason: "After aging enqueue times, one/two pair while an incompatible head remains waiting.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn widening_without_a_new_join_skips_an_incompatible_head() {
    let f = Fixture::new().await;
    f.raw.execute_unprepared("INSERT INTO rating_states(user_id,pool,rating,completed) VALUES ('head','ten_minute_30',3000,1),('one','ten_minute_30',1500,1),('two','ten_minute_30',1650,1)").await.unwrap();
    f.join("head", POOL).await;
    f.join("one", POOL).await;
    f.join("two", POOL).await;
    assert!(f.store.tick(POOL).await.unwrap().is_none());
    f.raw
        .execute_unprepared(
            "UPDATE matchmaking_tickets SET enqueued_at=enqueued_at-30000 WHERE state='waiting'",
        )
        .await
        .unwrap();
    let game = f.store.tick(POOL).await.unwrap().unwrap();
    let record = f.store.load_game(&game).await.unwrap();
    assert!(record.participants.side(&uid("one")).is_ok());
    assert!(record.participants.side(&uid("two")).is_ok());
    assert!(
        matches!(f.store.current(&uid("head")).await.unwrap(),Participation::Active(t) if matches!(t.state,TicketState::Waiting(_)))
    );
}
}
