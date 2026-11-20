use super::*;

#[traceability::targets(game_persistence::SeaOrmStore::heartbeat)]
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
    assert_eq!(
        recovered.heartbeat(&uid("one"), id, stale).await.unwrap(),
        result
    );
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
    assert_eq!(
        recovered
            .heartbeat(&uid("one"), id, seq)
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

#[traceability::targets(game_persistence::SeaOrmStore::heartbeat)]
#[tokio::test]
async fn terminal_heartbeats_survive_reconnect_without_creating_participation() {
    let f = Fixture::new().await;
    let id = f.join("one", POOL).await;
    let refused = TicketId::new();
    let refusal = f
        .store
        .join(uid("one"), refused, POOL)
        .await
        .unwrap()
        .ticket;
    let cancelled = f.store.cancel(&uid("one"), id).await.unwrap();
    let recovered = SeaOrmStore::connect(f._db.url()).await.unwrap();
    let sequence = HeartbeatSequence::new(4).unwrap();
    for terminal in [cancelled, refusal] {
        assert_eq!(
            recovered
                .heartbeat(&uid("one"), terminal.id, sequence)
                .await
                .unwrap(),
            terminal
        );
        assert_eq!(
            recovered
                .heartbeat(&uid("one"), terminal.id, sequence)
                .await
                .unwrap(),
            terminal
        );
    }
    assert_eq!(
        recovered.current(&uid("one")).await.unwrap(),
        Participation::Idle
    );
    let tickets = f.count("matchmaking_tickets").await;
    assert_eq!(
        recovered.heartbeat(&uid("other"), id, sequence).await,
        Err(MatchmakingError::NotFound)
    );
    assert_eq!(
        recovered
            .heartbeat(&uid("other"), TicketId::new(), sequence)
            .await,
        Err(MatchmakingError::NotFound)
    );
    assert_eq!(
        recovered.current(&uid("other")).await.unwrap(),
        Participation::Idle
    );
    assert_eq!(f.count("matchmaking_tickets").await, tickets);
}

#[traceability::targets(game_persistence::SeaOrmStore::heartbeat)]
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
    assert!(
        matches!(matched.state, TicketState::Matched { ref game_id, ready: false, .. } if game_id == &game)
    );
    let recovered = SeaOrmStore::connect(f._db.url()).await.unwrap();
    assert_eq!(
        recovered
            .heartbeat(&uid("one"), one, sequence)
            .await
            .unwrap(),
        matched
    );
    assert_eq!(
        recovered
            .heartbeat(&uid("one"), one, HeartbeatSequence::new(2).unwrap())
            .await
            .unwrap(),
        matched
    );
    assert_eq!(f.count("games").await, 0);
    f.store.ready(&uid("two"), two).await.unwrap();
    assert_eq!(
        recovered
            .heartbeat(&uid("one"), one, HeartbeatSequence::new(3).unwrap())
            .await
            .unwrap(),
        matched
    );
    assert_eq!(f.count("games").await, 0);
    let playing = recovered.ready(&uid("one"), one).await.unwrap();
    assert!(matches!(playing.state, TicketState::Playing { ref game_id, .. } if game_id == &game));
    assert_eq!(
        recovered
            .heartbeat(&uid("one"), one, sequence)
            .await
            .unwrap(),
        playing
    );
    GameService::new(f.store.clone())
        .resign(
            &game,
            Resign {
                request_id: RequestId::new("heartbeat-finish").unwrap(),
                user_id: uid("one"),
                expected_seq: 0,
            },
        )
        .await
        .unwrap();
    let finished = recovered
        .heartbeat(&uid("one"), one, HeartbeatSequence::new(4).unwrap())
        .await
        .unwrap();
    assert!(
        matches!(finished.state, TicketState::Finished { ref game_id, .. } if game_id == &game)
    );
    assert_eq!(
        recovered
            .heartbeat(&uid("one"), one, sequence)
            .await
            .unwrap(),
        finished
    );
    assert_eq!(
        recovered.current(&uid("one")).await.unwrap(),
        Participation::Idle
    );
}
