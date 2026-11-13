use game_application::{CommittedEvent, DomainEvent};
use game_messaging::Notification;
use serde_json::Value;

#[derive(Clone)]
pub(crate) enum ConnectionState {
    Bootstrapping,
    Live(Cursor),
    Closing,
}

#[derive(Clone)]
pub(crate) struct Cursor {
    boundary: u64,
    last_seq: u64,
    move_event: Option<CommittedEvent>,
    finish_event: Option<CommittedEvent>,
    terminal: bool,
}

pub(crate) enum Decision {
    Deliver,
    Ignore,
    Resync,
}

impl ConnectionState {
    pub(crate) fn bootstrapped(&mut self, seq: u64, terminal: bool) {
        *self = Self::Live(Cursor {
            boundary: seq,
            last_seq: seq,
            move_event: None,
            finish_event: None,
            terminal,
        });
    }

    pub(crate) fn accept(&mut self, notification: &Notification) -> Decision {
        let Self::Live(cursor) = self else {
            return Decision::Resync;
        };
        cursor.accept(&notification.0)
    }
}

impl Cursor {
    fn accept(&mut self, event: &CommittedEvent) -> Decision {
        if event.seq <= self.boundary || event.seq < self.last_seq {
            return Decision::Ignore;
        }
        if event.seq == self.last_seq {
            let prior = match event.event {
                DomainEvent::MoveCommitted { .. } => self.move_event.as_ref(),
                DomainEvent::GameFinished { .. } => self.finish_event.as_ref(),
            };
            if let Some(prior) = prior {
                return if prior == event {
                    Decision::Ignore
                } else {
                    Decision::Resync
                };
            }
            if let (
                Some(
                    prior @ CommittedEvent {
                        event:
                            DomainEvent::MoveCommitted {
                                result: Some(result),
                                ..
                            },
                        ..
                    },
                ),
                DomainEvent::GameFinished { result: finish },
            ) = (&self.move_event, &event.event)
                && result == finish
                && prior.request_id == event.request_id
                && prior.user_id == event.user_id
            {
                self.finish_event = Some(event.clone());
                self.terminal = true;
                return Decision::Deliver;
            }
            return Decision::Resync;
        }
        if self.terminal || event.seq != self.last_seq + 1 {
            return Decision::Resync;
        }
        if matches!(&event.event, DomainEvent::GameFinished { result } if result.reason != shogi_domain::FinishReason::Resignation)
        {
            return Decision::Resync;
        }
        self.last_seq = event.seq;
        self.move_event = None;
        self.finish_event = None;
        match &event.event {
            DomainEvent::MoveCommitted { result, .. } => {
                self.move_event = Some(event.clone());
                self.terminal = result.is_some();
            }
            DomainEvent::GameFinished { .. } => {
                self.finish_event = Some(event.clone());
                self.terminal = true;
            }
        }
        Decision::Deliver
    }
}

pub(crate) fn event_json(notification: &Notification) -> Value {
    let event = &notification.0;
    let base = serde_json::json!({
        "type": "event", "event_id": format!("{}:{}:{}", hex(event.game_id.as_str()), event.seq, match event.event { DomainEvent::MoveCommitted { .. } => "move_committed", DomainEvent::GameFinished { .. } => "game_finished" }),
        "game_id": event.game_id.as_str(), "seq": event.seq.to_string(),
    });
    let mut object = base.as_object().unwrap().clone();
    match event.event {
        DomainEvent::MoveCommitted {
            movement,
            in_check,
            result,
        } => {
            object.insert("kind".into(), "move_committed".into());
            object.insert("usi".into(), movement.to_string().into());
            object.insert("in_check".into(), in_check.into());
            object.insert("result".into(), result_json(result));
        }
        DomainEvent::GameFinished { result } => {
            object.insert("kind".into(), "game_finished".into());
            object.insert("result".into(), result_json(Some(result)));
        }
    }
    Value::Object(object)
}

fn hex(value: &str) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(&mut out, "{byte:02x}").unwrap();
    }
    out
}

pub(crate) fn result_json(result: Option<shogi_domain::GameResult>) -> Value {
    match result {
        None => Value::Null,
        Some(result) => serde_json::json!({
            "winner": match result.winner { shogi_domain::Player::Black => "black", shogi_domain::Player::White => "white" },
            "reason": match result.reason { shogi_domain::FinishReason::Checkmate => "checkmate", shogi_domain::FinishReason::Resignation => "resignation" },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_application::{GameId, RequestId, UserId};
    use shogi_domain::{FinishReason, GameResult, Player};
    fn event(seq: u64, fact: DomainEvent) -> Notification {
        Notification(CommittedEvent {
            game_id: GameId::new("g").unwrap(),
            request_id: RequestId::new("r").unwrap(),
            user_id: UserId::new("u").unwrap(),
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
}
