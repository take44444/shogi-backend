use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。データベースの有効期限を強制的に切らす。",
        reason: "同じシーケンスでは同じチケットをそのまま返す。強制失効後の新しいハートビートは Expired となり、参加を復活させない。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "reads", scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。データベースの有効期限を強制的に切らす。", relationship: crate::trace_support::Relationship::Asserts, reason: "有効期限を強制的に切らすと、現在の参加状態が Idle になる。" }],
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
