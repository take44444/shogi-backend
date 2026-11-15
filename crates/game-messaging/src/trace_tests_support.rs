use super::*;
use game_application::{RequestId, UserId};
use serde_json::json;
fn event(game: &str, event: DomainEvent) -> CommittedEvent {
    CommittedEvent {
        game_id: GameId::new(game).unwrap(),
        request_id: RequestId::new("r\"\0").unwrap(),
        user_id: UserId::new("user").unwrap(),
        seq: 4,
        event,
    }
}

include!(
    "trace/realtime-game-feed/event-frame/decoder_roundtrips_publisher_and_rejects_mismatches.rs"
);

#[path = "trace/durable-event-dispatch/wire-format/tests.rs"]
mod wire_format;
