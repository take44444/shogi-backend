use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。 実際の購読者がコミットを受信する間、SnapshotHoldで初期応答を保留する。",
        reason: "連番0のスナップショットを確定した状態で保留し、実際の購読者を確認する。連番1をコミット・配信した後、スナップショット0、イベント1の順に受信する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "realtime-game-feed", contract: "snapshot-frame", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。 実際の購読者がコミットを受信する間、SnapshotHoldで初期応答を保留する。", relationship: crate::trace_support::Relationship::Asserts, reason: "後続の着手フレームより先に、連番0のスナップショットを受信する。" }],
    };
#[tokio::test]
async fn committed_move_during_initial_snapshot_arrives_after_the_frozen_snapshot() {
    let (captured, snapshot_ready) = oneshot::channel();
    let (release_snapshot, release) = oneshot::channel();
    let mut services =
        Services::with_snapshot_hold(Limits::default(), Some(SnapshotHold { captured, release }))
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
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。",
        reason: "実際の3本のWebSocketが、初期スナップショット、コミット済みの着手2件、投了を受信する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "realtime-game-feed", contract: "snapshot-frame", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "接続した参加者全員と観戦者が、期待したgame_id・連番を持つスナップショットフレームを受信する。" }, crate::trace_support::Evidence { feature: "realtime-game-feed", contract: "event-frame", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "対局者と観戦者が、着手・投了の種別、USI・結果、user_id・request_idが含まれないことを検証する。" }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "command-input", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Exercises, reason: "有効な参加者の着手・投了コマンドが送られ、一致する受理結果を返す。" }, crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "command-response", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "受理されたリクエストの識別情報と連番が、参加者のコマンドに一致する。" }],
    };
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
        assert_eq!(
            event["result"],
            json!({"winner":"black", "reason":"resignation"})
        );
    }
    assert_eq!(services.snapshot().await.seq, 3);
    services.stop().await;
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。 永続化された受理の後に接続を切り、元の識別情報で再試行する。",
        reason: "再接続時のスナップショットは連番1で、再試行は元の受理済み連番1を返し、永続化されたイベントは1件となる。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "realtime-session-commands-limits", contract: "command-response", scope: "実際のHTTP・WebSocketゲートウェイ、ユーザー用HTTPサービス、対局用gRPCサービス、PostgreSQL、Valkeyを使う。外部OIDCプロバイダーはテスト用の代替実装を使う。", relationship: crate::trace_support::Relationship::Asserts, reason: "再接続後の再試行は、イベントを重複させずに元の受理済み連番を返す。" }],
    };
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
}
