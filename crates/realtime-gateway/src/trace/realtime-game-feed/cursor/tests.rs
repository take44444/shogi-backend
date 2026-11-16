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
crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "人工的なコミット済み通知を使ったカーソルの状態遷移の直接検証。ソケットも、詰みに至る実際の手順も使わない。",
        reason: "着手と対応する詰みの終局を順に配信し、完全に同じ重複を無視し、同じ連番で競合する着手では再同期を求める。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "カーソルの境界・連番欠落を直接検証するテスト用データ。ブローカーやソケットは使わない。",
        reason: "古い境界イベントは無視し、対応する着手がない詰みの終局と連番の欠落では再同期を求める。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
