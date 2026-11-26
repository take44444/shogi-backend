use super::*;
use game_application::{GameId, RequestId, UserId};
use shogi_domain::{FinishReason, GameResult, Player};
fn event(seq: u64, fact: DomainEvent) -> Notification {
    Notification(CommittedEvent {
        clock: game_application::ClockState::start(
            game_application::TimeControl::for_pool(game_application::rating::Pool::TenMinute30),
            game_application::Timestamp::new(0).unwrap(),
        )
        .unwrap(),
        evaluated_at: game_application::Timestamp::new(0).unwrap(),
        game_id: GameId::new("g").unwrap(),
        origin: game_application::EventOrigin::User {
            request_id: RequestId::new("r").unwrap(),
            user_id: UserId::new("u").unwrap(),
        },
        seq,
        event: fact,
    })
}
#[test]
fn same_sequence_checkmate_pair_and_conflicts() {
    let result = GameResult {
        winner: Player::Black,
        reason: FinishReason::Checkmate,
    };
    let move_event = event(
        1,
        DomainEvent::MoveCommitted {
            movement: "7g7f".parse().unwrap(),
            in_check: true,
            result: Some(result),
        },
    );
    let finish = event(1, DomainEvent::GameFinished { result });
    let mut state = ConnectionState::Bootstrapping;
    state.bootstrapped(0, false);
    assert!(matches!(state.accept(&move_event), Decision::Deliver));
    assert!(matches!(state.accept(&move_event), Decision::Ignore));
    assert!(matches!(state.accept(&finish), Decision::Deliver));
    assert!(matches!(state.accept(&finish), Decision::Ignore));
    let changed = event(
        1,
        DomainEvent::MoveCommitted {
            movement: "2g2f".parse().unwrap(),
            in_check: true,
            result: Some(result),
        },
    );
    assert!(matches!(state.accept(&changed), Decision::Resync));
}

#[test]
fn gaps_and_unpaired_checkmate_finish_require_resync() {
    let mut state = ConnectionState::Bootstrapping;
    state.bootstrapped(3, false);
    let result = GameResult {
        winner: Player::Black,
        reason: FinishReason::Checkmate,
    };
    assert!(matches!(
        state.accept(&event(3, DomainEvent::GameFinished { result })),
        Decision::Ignore
    ));
    assert!(matches!(
        state.accept(&event(4, DomainEvent::GameFinished { result })),
        Decision::Resync
    ));
    assert!(matches!(
        state.accept(&event(
            5,
            DomainEvent::MoveCommitted {
                movement: "7g7f".parse().unwrap(),
                in_check: false,
                result: None
            }
        )),
        Decision::Resync
    ));
}

#[test]
fn standalone_system_timeout_delivers_once_and_preserves_final_clock() {
    let mut notification = event(
        4,
        DomainEvent::GameFinished {
            result: GameResult {
                winner: Player::White,
                reason: FinishReason::Timeout,
            },
        },
    );
    notification.0.origin = game_application::EventOrigin::System;
    let mut state = ConnectionState::Bootstrapping;
    state.bootstrapped(3, false);
    assert!(matches!(state.accept(&notification), Decision::Deliver));
    assert!(matches!(state.accept(&notification), Decision::Ignore));
    let value = event_json(&notification);
    assert_eq!(value["result"]["reason"], "timeout");
    assert!(value.get("clock").is_some());
    assert!(value.get("evaluated_at_ms").is_some());
    assert!(value.get("server_now_ms").is_none());
}
