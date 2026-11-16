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
        ("schema_version", json!(2)),
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
