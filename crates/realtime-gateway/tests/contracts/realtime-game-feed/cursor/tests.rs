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
            game_id: game.clone(),
            request_id: RequestId::new("missing-sequence").unwrap(),
            user_id: UserId::new(services.black.id.clone()).unwrap(),
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
