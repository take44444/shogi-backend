use super::*;

#[test]
fn version_one_json_has_explicit_moves_results_and_identity() {
    let result = GameResult {
        winner: Player::White,
        reason: FinishReason::Checkmate,
    };
    let value: serde_json::Value = serde_json::from_slice(
        &encode(&event(
            "game",
            DomainEvent::MoveCommitted {
                movement: "2b3c+".parse().unwrap(),
                in_check: true,
                result: Some(result),
            },
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        value,
        json!({"schema_version":1,"event_id":"67616d65:4:move_committed", "type":"move_committed",
            "game_id":"game","seq":4,"request_id":"r\"\0","user_id":"user","movement":"2b3c+","in_check":true,
            "result":{"winner":"white","reason":"checkmate"}})
    );
    let end: serde_json::Value = serde_json::from_slice(
        &encode(&event(
            "game",
            DomainEvent::GameFinished {
                result: GameResult {
                    winner: Player::Black,
                    reason: FinishReason::Resignation,
                },
            },
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(end["event_id"], "67616d65:4:game_finished");
    assert_eq!(
        end["result"],
        json!({"winner":"black","reason":"resignation"})
    );
    assert!(end.get("movement").is_none());
    assert!(end.get("in_check").is_none());
    let drop: serde_json::Value = serde_json::from_slice(
        &encode(&event(
            "game",
            DomainEvent::MoveCommitted {
                movement: "B*4e".parse().unwrap(),
                in_check: false,
                result: None,
            },
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(drop["movement"], "B*4e");
    assert!(drop["result"].is_null());
}

#[test]
fn channels_encode_every_id_byte_without_collisions_or_glob_metacharacters() {
    let inputs = ["a.b", "a*b", "a>b", "棋\0\"", "6162", "ab"];
    let subjects: std::collections::HashSet<_> = inputs
        .into_iter()
        .map(|id| {
            let event = event(
                id,
                DomainEvent::GameFinished {
                    result: GameResult {
                        winner: Player::Black,
                        reason: FinishReason::Resignation,
                    },
                },
            );
            let encoded = encode(&event).unwrap();
            let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(value["game_id"], id);
            let subject = game_channel(&event.game_id);
            assert_eq!(subject.split('.').count(), 4);
            assert!(
                subject
                    .split('.')
                    .nth(2)
                    .unwrap()
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit())
            );
            subject
        })
        .collect();
    assert_eq!(subjects.len(), inputs.len());
    assert_eq!(
        game_channel(&GameId::new("棋\0").unwrap()),
        "shogi.games.e6a38b00.events"
    );
}
