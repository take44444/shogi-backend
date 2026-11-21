use super::*;

pub(super) fn event(game: &str, event: DomainEvent) -> CommittedEvent {
    CommittedEvent {
        game_id: GameId::new(game).unwrap(),
        request_id: RequestId::new("r\"\0").unwrap(),
        user_id: UserId::new("user").unwrap(),
        seq: 4,
        event,
    }
}
