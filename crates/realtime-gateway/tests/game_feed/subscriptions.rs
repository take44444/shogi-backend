use super::*;

#[traceability::targets(game_messaging::ValkeySubscription::next)]
#[tokio::test]
async fn bounded_valkey_subscription_overflow_is_terminal() {
    let url =
        std::env::var("TEST_VALKEY_URL").expect("set TEST_VALKEY_URL to a real Valkey server");
    let game = GameId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let mut subscription = ValkeySubscription::subscribe(&url, &game, 1).await.unwrap();
    let mut publisher = ValkeyEventPublisher::connect(&url).await.unwrap();
    for seq in 1..=8 {
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
                    request_id: RequestId::new(format!("overflow-{seq}")).unwrap(),
                    user_id: UserId::new("black").unwrap(),
                },
                seq,
                event: DomainEvent::MoveCommitted {
                    movement: "7g7f".parse().unwrap(),
                    in_check: false,
                    result: None,
                },
            })
            .await
            .unwrap();
    }
    assert!(timeout(WAIT, subscription.next()).await.unwrap().is_err());
    assert!(timeout(WAIT, subscription.next()).await.unwrap().is_err());
}

#[traceability::targets(game_messaging::ValkeySubscription::subscribe)]
#[tokio::test]
async fn one_notification_fits_capacity_one_before_the_first_read() {
    let mut services = Services::start().await;
    let game = GameId::new(services.game.clone()).unwrap();
    let mut subscription = ValkeySubscription::subscribe(&services.valkey_url, &game, 1)
        .await
        .unwrap();
    services
        .client
        .make_move(proto::MakeMoveRequest {
            game_id: services.game.clone(),
            request_id: "single-buffer-slot".into(),
            expected_seq: Some(0),
            usi: "7g7f".into(),
            user_id: services.black.id.clone(),
        })
        .await
        .unwrap();
    services.publish().await;
    assert_eq!(
        timeout(WAIT, subscription.next())
            .await
            .unwrap()
            .unwrap()
            .0
            .seq,
        1
    );
    services.stop().await;
}
