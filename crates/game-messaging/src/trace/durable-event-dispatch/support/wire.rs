#![cfg(test)]

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

#[path = "../../realtime-game-feed/event-frame/tests.rs"]
mod realtime_game_feed_event_frame;

#[path = "../wire-format/tests.rs"]
mod wire_format;
