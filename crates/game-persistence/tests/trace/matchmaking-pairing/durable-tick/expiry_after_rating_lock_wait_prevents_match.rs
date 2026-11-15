crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport. Explicit held rating locks and forced expiry.",
        reason: "Tickets expire while blocked on rating locks; tick returns None and creates no game.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn expiry_after_rating_lock_wait_prevents_match() {
    let f = Fixture::new().await;
    let one = f.join("one", POOL).await;
    f.join("two", POOL).await;
    f.raw.execute_unprepared("UPDATE matchmaking_tickets SET expires_at=floor(extract(epoch FROM clock_timestamp())*1000)::bigint+150").await.unwrap();
    let tx = f.raw.begin().await.unwrap();
    tx.execute_unprepared("SELECT * FROM rating_states ORDER BY user_id FOR UPDATE")
        .await
        .unwrap();
    let store = f.store.clone();
    let pending = tokio::spawn(async move { store.tick(POOL).await });
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    tx.commit().await.unwrap();
    assert_eq!(pending.await.unwrap().unwrap(), None);
    assert_eq!(f.count("games").await, 0);
    assert_eq!(
        f.store.ticket(&uid("one"), one).await.unwrap().state,
        TicketState::Expired
    );
}
}
