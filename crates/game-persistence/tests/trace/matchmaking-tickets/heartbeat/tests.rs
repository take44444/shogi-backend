use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport. Database expiry is forced.",
        reason: "Duplicate sequence returns exactly the same ticket; a newer heartbeat after forced expiry produces Expired and does not revive admission.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "reads", scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport. Database expiry is forced.", relationship: crate::trace_support::Relationship::Asserts, reason: "After forced expiry current becomes Idle." }],
    };
#[tokio::test]
async fn heartbeat_retries_do_not_extend_and_expired_waiting_never_revives() {
    let f = Fixture::new().await;
    let id = f.join("one", POOL).await;
    let seq = HeartbeatSequence::new(1).unwrap();
    let result = f.store.heartbeat(&uid("one"), id, seq).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    assert_eq!(
        f.store.heartbeat(&uid("one"), id, seq).await.unwrap(),
        result
    );
    f.raw
        .execute_unprepared("UPDATE matchmaking_tickets SET enqueued_at=0, expires_at=1")
        .await
        .unwrap();
    assert_eq!(
        f.store
            .heartbeat(&uid("one"), id, HeartbeatSequence::new(2).unwrap())
            .await
            .unwrap()
            .state,
        TicketState::Expired
    );
    assert!(!f.store.join(uid("one"), id, POOL).await.unwrap().created);
    assert_eq!(
        f.store.current(&uid("one")).await.unwrap(),
        Participation::Idle
    );
}
}
