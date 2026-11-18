use super::*;

#[test]
fn rejection_preserves_all_observable_state() {
    let mut game = Game::new();
    reject(&mut game, Player::White, "3c3d", CommandError::WrongTurn);
    for illegal in [
        "7g7e", "7f7e", "3c3d", "7g7g", "7g6g", "8h3c", "7g7f+", "P*5e", "5i5h+",
    ] {
        reject(&mut game, Player::Black, illegal, CommandError::IllegalMove);
    }
    play(&mut game, "7g7f");
    reject(&mut game, Player::Black, "2g2f", CommandError::WrongTurn);
    play(&mut game, "3c3d");
}

#[test]
fn checkmate_finishes_and_finished_precedes_turn_or_legality() {
    let mut game = Game::from_sfen("3pkp3/3p1p3/4G4/9/9/9/9/9/K3R4 b - 1");
    let result = GameResult {
        winner: Player::Black,
        reason: FinishReason::Checkmate,
    };
    assert_eq!(
        play(&mut game, "5c5b"),
        MoveOutcome {
            in_check: true,
            result: Some(result)
        }
    );
    assert_eq!(game.turn(), Player::White);
    assert!(game.in_check());
    assert!(game.legal_moves().is_empty());
    for player in [Player::Black, Player::White] {
        reject(&mut game, player, "5a5b", CommandError::GameFinished);
        let before = snapshot(&game);
        assert_eq!(game.resign(player), Err(CommandError::GameFinished));
        assert_eq!(snapshot(&game), before);
    }
}

#[test]
fn no_legal_moves_without_check_is_not_checkmate() {
    let game = Game::from_sfen("8k/6R2/7R1/9/9/9/9/9/K8 w - 1");
    assert!(!game.in_check());
    assert!(game.legal_moves().is_empty());
    assert_eq!(game.result(), None);
    let mut game = Game::from_sfen("8k/6R2/9/7R1/9/9/9/9/K8 b - 1");
    assert_eq!(
        play(&mut game, "2d2c"),
        MoveOutcome {
            in_check: false,
            result: None
        }
    );
    assert!(game.legal_moves().is_empty());
    assert_eq!(
        game.resign(Player::White).unwrap().reason,
        FinishReason::Resignation
    );
}

#[test]
fn repeated_positions_and_perpetual_check_remain_playable() {
    let mut game = Game::from_sfen("4k4/9/9/9/9/9/9/9/4K4 b - 1");
    let initial = game.position();
    for _ in 0..5 {
        for movement in ["5i4i", "5a4a", "4i5i", "4a5a"] {
            play(&mut game, movement);
        }
        assert_eq!(game.position(), initial);
        assert_eq!(game.result(), None);
    }
    let mut game = Game::from_sfen("4k4/9/5R3/9/9/9/9/9/K8 b - 1");
    for _ in 0..5 {
        for movement in ["4c5c", "5a4a", "5c4c", "4a5a"] {
            play(&mut game, movement);
        }
        assert_eq!(game.result(), None);
    }
}

#[test]
fn public_errors_implement_standard_error() {
    fn error<T: std::error::Error>(value: T) {
        assert!(!value.to_string().is_empty());
    }
    error(InvalidSquare);
    error(ParseUsiError);
    for e in [
        CommandError::GameFinished,
        CommandError::WrongTurn,
        CommandError::IllegalMove,
        CommandError::RulesFailure,
    ] {
        error(e);
    }
}
