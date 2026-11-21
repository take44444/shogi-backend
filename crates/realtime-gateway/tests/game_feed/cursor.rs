use super::*;

#[tokio::test]
async fn malformed_or_gapped_broker_notifications_require_a_new_connection() {
    let services = Services::start().await;
    let game = GameId::new(services.game.clone()).unwrap();
    let mut socket = services.connect(&services.viewer).await;
    initial(&mut socket, &services.game, "0").await;
    let mut publisher = ValkeyEventPublisher::connect(&services.valkey_url)
        .await
        .unwrap();
    publisher
        .publish(&CommittedEvent {
            clock: game_application::ClockState::start(
                game_application::TimeControl::for_pool(
                    game_application::rating::Pool::TenMinute30,
                ),
                game_application::Timestamp::new(0).unwrap(),
            )
            .unwrap(),
            evaluated_at: game_application::Timestamp::new(0).unwrap(),
            game_id: game.clone(),
            origin: game_application::EventOrigin::User {
                request_id: RequestId::new("missing-sequence").unwrap(),
                user_id: UserId::new(services.black.id.clone()).unwrap(),
            },
            seq: 2,
            event: DomainEvent::MoveCommitted {
                movement: "7g7f".parse().unwrap(),
                in_check: false,
                result: None,
            },
        })
        .await
        .unwrap();
    closed(&mut socket).await;
    let mut socket = services.connect(&services.viewer).await;
    initial(&mut socket, &services.game, "0").await;
    let mut connection = redis::Client::open(services.valkey_url.clone())
        .unwrap()
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    let subscribers: i64 = redis::cmd("PUBLISH")
        .arg(game_channel(&game))
        .arg(r#"{"schema_version":999}"#)
        .query_async(&mut connection)
        .await
        .unwrap();
    assert!(subscribers >= 1);
    closed(&mut socket).await;
    services.stop().await;
}

#[tokio::test]
async fn timeout_notification_and_reconnect_show_the_same_system_result_and_stopped_clock() {
    let mut services = Services::start().await;
    let mut socket = services.connect(&services.viewer).await;
    initial(&mut socket, &services.game, "0").await;
    let raw = Database::connect(services.game_database.url())
        .await
        .unwrap();
    raw.execute_unprepared("UPDATE games SET created_at=created_at-631000,clock_started=clock_started-631000,clock_deadline=clock_deadline-631000").await.unwrap();
    let service = game_application::GameService::new(services.games.clone());
    assert_eq!(service.poll_timeouts(16).await.unwrap(), 1);
    services.publish().await;
    let result = receive(&mut socket).await;
    assert_eq!(result["kind"], "game_finished");
    assert_eq!(result["seq"], "1");
    assert_eq!(result["result"]["reason"], "timeout");
    drop(socket);
    let mut socket = services.connect(&services.viewer).await;
    let snapshot = receive(&mut socket).await;
    assert_eq!(snapshot["seq"], "1");
    assert_eq!(snapshot["result"]["reason"], "timeout");
    assert_eq!(snapshot["clock"]["state"], "stopped");
    raw.close().await.unwrap();
    services.stop().await;
}
