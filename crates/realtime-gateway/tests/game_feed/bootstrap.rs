use super::*;

#[tokio::test]
async fn committed_move_during_initial_snapshot_arrives_after_the_frozen_snapshot() {
    let (captured, snapshot_ready) = oneshot::channel();
    let (release_snapshot, release) = oneshot::channel();
    let mut services = Services::with_snapshot_hold(
        Limits {
            queue: 2,
            ..Limits::default()
        },
        Some(SnapshotHold { captured, release }),
    )
    .await;
    let mut viewer = services.connect(&services.viewer).await;
    assert_eq!(timeout(WAIT, snapshot_ready).await.unwrap().unwrap(), 0);
    let mut broker = redis::Client::open(services.valkey_url.clone())
        .unwrap()
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    let game = GameId::new(services.game.clone()).unwrap();
    let (_, subscribers): (String, i64) = redis::cmd("PUBSUB")
        .arg("NUMSUB")
        .arg(game_channel(&game))
        .query_async(&mut broker)
        .await
        .unwrap();
    assert_eq!(
        subscribers, 1,
        "subscription is active while snapshot is held"
    );
    let accepted = services
        .client
        .make_move(proto::MakeMoveRequest {
            game_id: services.game.clone(),
            request_id: "during-bootstrap".into(),
            expected_seq: Some(0),
            usi: "7g7f".into(),
            user_id: services.black.id.clone(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(accepted.seq, 1);
    services.publish().await;
    assert_eq!(services.snapshot().await.seq, 1);
    release_snapshot.send(()).unwrap();
    initial(&mut viewer, &services.game, "0").await;
    let event = receive(&mut viewer).await;
    assert_eq!(event["type"], "event");
    assert_eq!(event["kind"], "move_committed");
    assert_eq!(event["seq"], "1");
    assert_eq!(event["usi"], "7g7f");
    services.stop().await;
}

#[tokio::test]
async fn participants_and_spectator_receive_committed_moves_and_resignation() {
    let mut services = Services::start().await;
    let mut black = services.connect(&services.black).await;
    let mut white = services.connect(&services.white).await;
    let mut viewer = services.connect(&services.viewer).await;
    for socket in [&mut black, &mut white, &mut viewer] {
        initial(socket, &services.game, "0").await;
    }
    send(&mut black, movement("black-move", "0", "7g7f")).await;
    let accepted = receive(&mut black).await;
    assert_eq!(accepted["type"], "accepted");
    assert_eq!(accepted["request_id"], "black-move");
    assert_eq!(accepted["seq"], "1");
    services.publish().await;
    for socket in [&mut black, &mut white, &mut viewer] {
        let event = receive(socket).await;
        assert_eq!(event["type"], "event");
        assert_eq!(event["kind"], "move_committed");
        assert_eq!(event["seq"], "1");
        assert_eq!(event["usi"], "7g7f");
        assert_eq!(event["clock"], accepted["clock"]);
        assert_eq!(event["clock"]["state"], "running");
        assert!(
            event["clock"]["deadline_at_ms"].as_i64().unwrap()
                > event["evaluated_at_ms"].as_i64().unwrap()
        );
        assert!(event.get("server_now_ms").is_none());
        assert!(event.get("request_id").is_none());
        assert!(event.get("user_id").is_none());
    }
    send(&mut white, movement("white-move", "1", "3c3d")).await;
    assert_eq!(receive(&mut white).await["seq"], "2");
    services.publish().await;
    for socket in [&mut black, &mut white, &mut viewer] {
        assert_eq!(receive(socket).await["usi"], "3c3d");
    }
    send(
        &mut white,
        json!({"type":"resign", "request_id":"end", "expected_seq":"2"}),
    )
    .await;
    assert_eq!(receive(&mut white).await["type"], "accepted");
    services.publish().await;
    for socket in [&mut black, &mut white, &mut viewer] {
        let event = receive(socket).await;
        assert_eq!(event["seq"], "3");
        assert_eq!(event["kind"], "game_finished");
        assert_eq!(event["clock"]["state"], "stopped");
        assert_eq!(
            event["result"],
            json!({"winner":"black", "reason":"resignation"})
        );
    }
    assert_eq!(services.snapshot().await.seq, 3);
    services.stop().await;
}

#[tokio::test]
async fn lost_acceptance_replays_original_receipt_after_reconnect_without_a_second_move() {
    let mut services = Services::start().await;
    let mut black = services.connect(&services.black).await;
    initial(&mut black, &services.game, "0").await;
    let command = movement("lost-response", "0", "7g7f");
    send(&mut black, command.clone()).await;
    timeout(WAIT, async {
        while services.snapshot().await.seq != 1 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    drop(black);
    let mut retry = services.connect(&services.black).await;
    initial(&mut retry, &services.game, "1").await;
    send(&mut retry, command).await;
    let accepted = receive(&mut retry).await;
    assert_eq!(accepted["type"], "accepted");
    assert_eq!(accepted["request_id"], "lost-response");
    assert_eq!(accepted["seq"], "1");
    assert_eq!(services.snapshot().await.seq, 1);
    services.publish().await;
    assert!(
        !services.publisher.publish_next().await.unwrap(),
        "retry does not enqueue another event"
    );
    services.stop().await;
}

#[tokio::test]
async fn snapshot_hold_uses_one_bounded_notification_queue_and_overflow_requires_resync() {
    let (captured, snapshot_ready) = oneshot::channel();
    let (release_snapshot, release) = oneshot::channel();
    let mut services = Services::with_snapshot_hold(
        Limits {
            queue: 2,
            ..Limits::default()
        },
        Some(SnapshotHold { captured, release }),
    )
    .await;
    let mut socket = services.connect(&services.viewer).await;
    assert_eq!(timeout(WAIT, snapshot_ready).await.unwrap().unwrap(), 0);
    let game = GameId::new(services.game.clone()).unwrap();
    let mut observer = ValkeySubscription::subscribe(&services.valkey_url, &game, 8)
        .await
        .unwrap();
    services
        .client
        .make_move(proto::MakeMoveRequest {
            game_id: services.game.clone(),
            request_id: "bootstrap-overflow".into(),
            expected_seq: Some(0),
            usi: "7g7f".into(),
            user_id: services.black.id.clone(),
        })
        .await
        .unwrap();
    services.publish().await;
    let event = observer.next().await.unwrap().0;
    let mut publisher = ValkeyEventPublisher::connect(&services.valkey_url)
        .await
        .unwrap();
    for _ in 0..2 {
        publisher.publish(&event).await.unwrap();
        assert_eq!(observer.next().await.unwrap().0, event);
    }
    assert!(
        timeout(Duration::from_millis(50), socket.next())
            .await
            .is_err(),
        "snapshot remains held while the subscription buffers three pushes"
    );
    release_snapshot.send(()).unwrap();
    initial(&mut socket, &services.game, "0").await;
    assert_eq!(
        receive(&mut socket).await,
        json!({"type":"resync_required","reason":"subscription_lost"})
    );
    closed(&mut socket).await;
    services.stop().await;
}
