use super::*;

#[traceability::targets(
    game_persistence::SeaOrmStore::join,
    game_persistence::SeaOrmStore::cancel
)]
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
        restarted
            .heartbeat(&uid("other"), first, HeartbeatSequence::new(1).unwrap())
            .await,
        Err(MatchmakingError::NotFound)
    );
    assert_eq!(
        restarted.current(&uid("other")).await.unwrap(),
        Participation::Idle
    );
    assert_eq!(
        restarted
            .join(uid("one"), refused, Pool::Byoyomi30)
            .await
            .unwrap(),
        r
    );
    let next = f.join("one", POOL).await;
    assert_eq!(
        f.store.join(uid("one"), next, Pool::Byoyomi30).await,
        Err(MatchmakingError::PoolConflict)
    );
    f.store.cancel(&uid("one"), first).await.unwrap();
    let current = active(&f.store, &uid("one")).await;
    assert_eq!(current.id, next);
    assert!(matches!(current.state, TicketState::Waiting(_)));
}
