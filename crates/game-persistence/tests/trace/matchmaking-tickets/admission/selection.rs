use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。",
        reason: "保存済みの already_waiting による拒否は、取消後も再接続前に再現される。再接続後も、参加前の取消記録は両プールで Cancelled のまま残り、プールを変えた再試行は競合する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "heartbeat", scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。", relationship: crate::trace_support::Relationship::Asserts, reason: "別の所有者へのハートビートは NotFound となり、参加状態を作らない。" }, crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "cancel", scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。", relationship: crate::trace_support::Relationship::Asserts, reason: "参加前の取消は再接続後も両方のプールへの参加時も残る。古い識別子を取り消しても、新しいチケットは待機を続ける。" }],
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
        restarted.heartbeat(&uid("other"), first, HeartbeatSequence::new(1).unwrap()).await,
        Err(MatchmakingError::NotFound)
    );
    assert_eq!(restarted.current(&uid("other")).await.unwrap(), Participation::Idle);
    assert_eq!(restarted.join(uid("one"), refused, Pool::Byoyomi30).await.unwrap(), r);
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
}
