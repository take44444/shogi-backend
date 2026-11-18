use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-messaging/gateway-subscription", relationship: crate::trace_support::Relationship::Asserts, scope: "実 Valkey と公開 ValkeySubscription API。ゲートウェイは起動しない。", reason: "上限付き購読キューの溢れで、後続の next も終端エラーを返すことを確認する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-messaging", "valkey"], fake: &[] }] };
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
                game_id: game.clone(),
                request_id: RequestId::new(format!("overflow-{seq}")).unwrap(),
                user_id: UserId::new("black").unwrap(),
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
}
