use crate::trace_support as trace;
use shogi_domain::{CommandError, FinishReason, Game, Player};

trace::boundary_case! {
    trace::BoundaryCase { evidence: &[trace::Evidence {
        contract: "shogi-domain/application-play",
        relationship: trace::Relationship::Asserts,
        scope: "公開 Game API と通常初期局面。private な SFEN fixture は使わない。",
        reason: "着手受理後の手番と、誤った手番・非合法手を拒否した後の公開局面の一致を確認する。",
        observation: trace::Observation::ProviderBoundary,
        real: &["shogi-domain"], fake: &[],
    }] };
    #[test]
    fn accepted_move_advances_turn_and_rejected_moves_preserve_position() {
        let mut game = Game::new();
        game.play(Player::Black, "7g7f".parse().unwrap()).unwrap();
        assert_eq!(game.turn(), Player::White);
        let before = game.position();
        assert_eq!(game.play(Player::Black, "2g2f".parse().unwrap()), Err(CommandError::WrongTurn));
        assert_eq!(game.position(), before);
        assert_eq!(game.play(Player::White, "3c3e".parse().unwrap()), Err(CommandError::IllegalMove));
        assert_eq!(game.position(), before);
        assert_eq!(game.turn(), Player::White);
        assert!(game.result().is_none());
    }
}

trace::boundary_case! {
    trace::BoundaryCase { evidence: &[trace::Evidence {
        contract: "shogi-domain/application-resign",
        relationship: trace::Relationship::Asserts,
        scope: "公開 Game API の手番外投了と終局後の操作。",
        reason: "相手を勝者として確定し、局面を保持したまま後続の着手と投了を拒否する。",
        observation: trace::Observation::ProviderBoundary,
        real: &["shogi-domain"], fake: &[],
    }] };
    #[test]
    fn off_turn_resignation_finishes_without_changing_the_position() {
        let mut game = Game::new();
        let before = game.position();
        let result = game.resign(Player::White).unwrap();
        assert_eq!(result.winner, Player::Black);
        assert_eq!(result.reason, FinishReason::Resignation);
        assert_eq!(game.position(), before);
        assert!(game.legal_moves().is_empty());
        assert_eq!(game.resign(Player::Black), Err(CommandError::GameFinished));
        assert_eq!(game.play(Player::Black, "7g7f".parse().unwrap()), Err(CommandError::GameFinished));
        assert_eq!(game.position(), before);
    }
}
