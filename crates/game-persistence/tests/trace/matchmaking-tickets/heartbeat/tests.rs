use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。データベースの有効期限を強制的に切らす。",
        reason: "再接続後も同じシーケンスや古いシーケンスでは期限を延長しない。強制失効後の新しいハートビートは Expired となり、参加を復活させない。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "current", scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。データベースの有効期限を強制的に切らす。", relationship: crate::trace_support::Relationship::Asserts, reason: "有効期限を強制的に切らすと、現在の参加状態が Idle になる。" }],
    };
#[tokio::test]
async fn heartbeat_retries_do_not_extend_and_expired_waiting_never_revives() {
    let f = Fixture::new().await;
    let id = f.join("one", POOL).await;
    let stale = HeartbeatSequence::new(1).unwrap();
    let seq = HeartbeatSequence::new(2).unwrap();
    let result = f.store.heartbeat(&uid("one"), id, seq).await.unwrap();
    let resumed = active(&f.store, &uid("one")).await;
    assert_eq!(resumed.id, id);
    assert_eq!(resumed, result);
    let recovered = SeaOrmStore::connect(f._db.url()).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    assert_eq!(
        recovered.heartbeat(&uid("one"), id, seq).await.unwrap(),
        result
    );
    assert_eq!(recovered.heartbeat(&uid("one"), id, stale).await.unwrap(), result);
    f.raw
        .execute_unprepared("UPDATE matchmaking_tickets SET enqueued_at=0, expires_at=1")
        .await
        .unwrap();
    assert_eq!(
        f.store
            .heartbeat(&uid("one"), id, HeartbeatSequence::new(3).unwrap())
            .await
            .unwrap()
            .state,
        TicketState::Expired
    );
    assert_eq!(recovered.heartbeat(&uid("one"), id, seq).await.unwrap().state, TicketState::Expired);
    assert!(!f.store.join(uid("one"), id, POOL).await.unwrap().created);
    assert_eq!(
        f.store.current(&uid("one")).await.unwrap(),
        Participation::Idle
    );
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL と別接続の SeaOrmStore を使い、取消済みと拒否済みのハートビートを再送する。HTTP や RPC は通らない。",
        reason: "取消済みと拒否済みのチケットは再接続後も終端状態を返す。他人や未知の識別子へのハートビートは NotFound となり、参加状態やチケットを作らない。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn terminal_heartbeats_survive_reconnect_without_creating_participation() {
    let f = Fixture::new().await;
    let id = f.join("one", POOL).await;
    let refused = TicketId::new();
    let refusal = f.store.join(uid("one"), refused, POOL).await.unwrap().ticket;
    let cancelled = f.store.cancel(&uid("one"), id).await.unwrap();
    let recovered = SeaOrmStore::connect(f._db.url()).await.unwrap();
    let sequence = HeartbeatSequence::new(4).unwrap();
    for terminal in [cancelled, refusal] {
        assert_eq!(recovered.heartbeat(&uid("one"), terminal.id, sequence).await.unwrap(), terminal);
        assert_eq!(recovered.heartbeat(&uid("one"), terminal.id, sequence).await.unwrap(), terminal);
    }
    assert_eq!(recovered.current(&uid("one")).await.unwrap(), Participation::Idle);
    let tickets = f.count("matchmaking_tickets").await;
    assert_eq!(recovered.heartbeat(&uid("other"), id, sequence).await, Err(MatchmakingError::NotFound));
    assert_eq!(recovered.heartbeat(&uid("other"), TicketId::new(), sequence).await, Err(MatchmakingError::NotFound));
    assert_eq!(recovered.current(&uid("other")).await.unwrap(), Participation::Idle);
    assert_eq!(f.count("matchmaking_tickets").await, tickets);
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL と別接続の SeaOrmStore を使い、待機中のハートビートをマッチ成立後と対局開始後に再送する。HTTP や RPC は通らない。",
        reason: "同じシーケンスの再送は現在の Matched と Playing を返す。マッチ後の新しいハートビートも確認と共有期限を変更せず、両者の明示的な ready だけが対局を開始する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn heartbeat_retries_follow_lifecycle_without_readiness_or_deadline_changes() {
    let f = Fixture::new().await;
    let one = f.join("one", POOL).await;
    let sequence = HeartbeatSequence::new(1).unwrap();
    let waiting = f.store.heartbeat(&uid("one"), one, sequence).await.unwrap();
    assert!(matches!(waiting.state, TicketState::Waiting(_)));
    let two = f.join("two", POOL).await;
    let game = f.store.tick(POOL).await.unwrap().unwrap();
    let matched = active(&f.store, &uid("one")).await;
    assert_eq!(matched.id, one);
    assert!(matches!(matched.state, TicketState::Matched { ref game_id, ready: false, .. } if game_id == &game));
    let recovered = SeaOrmStore::connect(f._db.url()).await.unwrap();
    assert_eq!(recovered.heartbeat(&uid("one"), one, sequence).await.unwrap(), matched);
    assert_eq!(recovered.heartbeat(&uid("one"), one, HeartbeatSequence::new(2).unwrap()).await.unwrap(), matched);
    assert_eq!(f.count("games").await, 0);
    f.store.ready(&uid("two"), two).await.unwrap();
    assert_eq!(recovered.heartbeat(&uid("one"), one, HeartbeatSequence::new(3).unwrap()).await.unwrap(), matched);
    assert_eq!(f.count("games").await, 0);
    let playing = recovered.ready(&uid("one"), one).await.unwrap();
    assert!(matches!(playing.state, TicketState::Playing { ref game_id, .. } if game_id == &game));
    assert_eq!(recovered.heartbeat(&uid("one"), one, sequence).await.unwrap(), playing);
    GameService::new(f.store.clone()).resign(&game, Resign {
        request_id: RequestId::new("heartbeat-finish").unwrap(), user_id: uid("one"), expected_seq: 0,
    }).await.unwrap();
    let finished = recovered.heartbeat(&uid("one"), one, HeartbeatSequence::new(4).unwrap()).await.unwrap();
    assert!(matches!(finished.state, TicketState::Finished { ref game_id, .. } if game_id == &game));
    assert_eq!(recovered.heartbeat(&uid("one"), one, sequence).await.unwrap(), finished);
    assert_eq!(recovered.current(&uid("one")).await.unwrap(), Participation::Idle);
}
}
