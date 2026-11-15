crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport.",
        reason: "The stored already_waiting refusal is replayed after cancellation before reconnect. After reconnect, cancel-before-join tombstones remain Cancelled in both pools; changed-pool retries conflict.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "reads", scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport.", relationship: crate::trace_support::Relationship::Asserts, reason: "Foreign owner ticket lookup returns NotFound and old cancelled identity cannot alter a newer waiting request." }, crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "cancel", scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport.", relationship: crate::trace_support::Relationship::Asserts, reason: "Cancellation before join survives reconnect and both pool joins; cancelling an old identity leaves a newer ticket waiting." }],
    };
#[tokio::test]
async fn durable_retry_refusal_and_cancel_before_join_survive_reconnect() {
    let f = Fixture::new().await;
    let first = f.join("one", POOL).await;
    let refused = TicketId::new();
    let r = f
        .store
        .join(uid("one"), refused, Pool::Byoyomi30)
        .await
        .unwrap();
    assert_eq!(
        r.ticket.state,
        TicketState::Refused(RefusalReason::AlreadyWaiting)
    );
    assert!(!r.created);
    f.store.cancel(&uid("one"), first).await.unwrap();
    assert_eq!(
        f.store
            .join(uid("one"), refused, Pool::Byoyomi30)
            .await
            .unwrap(),
        r
    );
    let early = TicketId::new();
    f.store.cancel(&uid("one"), early).await.unwrap();
    let restarted = SeaOrmStore::connect(f._db.url()).await.unwrap();
    for pool in Pool::ALL {
        assert_eq!(
            restarted
                .join(uid("one"), early, pool)
                .await
                .unwrap()
                .ticket
                .state,
            TicketState::Cancelled
        );
    }
    assert_eq!(
        restarted.ticket(&uid("other"), first).await,
        Err(MatchmakingError::NotFound)
    );
    let next = f.join("one", POOL).await;
    assert_eq!(
        f.store.join(uid("one"), next, Pool::Byoyomi30).await,
        Err(MatchmakingError::PoolConflict)
    );
    f.store.cancel(&uid("one"), first).await.unwrap();
    assert!(matches!(
        f.store.ticket(&uid("one"), next).await.unwrap().state,
        TicketState::Waiting(_)
    ));
}
}
