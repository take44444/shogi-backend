use super::*;

#[test]
fn decoder_roundtrips_publisher_and_rejects_mismatches() {
    let original = event(
        "game",
        DomainEvent::MoveCommitted {
            movement: "7g7f".parse().unwrap(),
            in_check: false,
            result: None,
        },
    );
    let encoded = encode(&original).unwrap();
    assert_eq!(
        decode_notification(&encoded, &original.game_id).unwrap().0,
        original
    );
    assert!(decode_notification(&encoded, &GameId::new("another").unwrap()).is_err());
    let mut value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    for (field, changed) in [
        ("schema_version", json!(3)),
        ("event_id", json!("wrong")),
        ("movement", json!("not a move")),
        ("seq", json!(0)),
    ] {
        let prior = value[field].clone();
        value[field] = changed;
        assert!(
            decode_notification(&serde_json::to_vec(&value).unwrap(), &original.game_id).is_err(),
            "{field}"
        );
        value[field] = prior;
    }
    value["unexpected"] = json!(true);
    assert!(decode_notification(&serde_json::to_vec(&value).unwrap(), &original.game_id).is_err());
}

#[test]
fn system_timeout_roundtrips_without_user_identity_and_rejects_fabricated_origins() {
    let event = CommittedEvent {
        clock: game_application::ClockState::start(
            game_application::TimeControl::for_pool(game_application::rating::Pool::TenMinute30),
            game_application::Timestamp::new(0).unwrap(),
        )
        .unwrap(),
        evaluated_at: game_application::Timestamp::new(0).unwrap(),
        game_id: GameId::new("clock").unwrap(),
        origin: EventOrigin::System,
        seq: 3,
        event: DomainEvent::GameFinished {
            result: GameResult {
                winner: Player::White,
                reason: FinishReason::Timeout,
            },
        },
    };
    let encoded = encode(&event).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(value["schema_version"], 2);
    assert_eq!(value["origin"], "system");
    assert!(value.get("request_id").is_none());
    assert!(value.get("user_id").is_none());
    assert_eq!(
        decode_notification(&encoded, &event.game_id).unwrap().0,
        event
    );
    value["origin"] = json!("user");
    value["request_id"] = json!("fake");
    value["user_id"] = json!("fake");
    assert!(decode_notification(&serde_json::to_vec(&value).unwrap(), &event.game_id).is_err());
}
