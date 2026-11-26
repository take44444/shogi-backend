use super::*;

pub(super) fn event(game: &str, event: DomainEvent) -> CommittedEvent {
    CommittedEvent {
        clock: game_application::ClockState::start(
            game_application::TimeControl::for_pool(game_application::rating::Pool::TenMinute30),
            game_application::Timestamp::new(0).unwrap(),
        )
        .unwrap(),
        evaluated_at: game_application::Timestamp::new(0).unwrap(),
        game_id: GameId::new(game).unwrap(),
        origin: game_application::EventOrigin::User {
            request_id: RequestId::new("r\"\0").unwrap(),
            user_id: UserId::new("user").unwrap(),
        },
        seq: 4,
        event,
    }
}
