use shogi_domain::{CommandError, FinishReason, Game, Player};

#[traceability::targets(shogi_domain::Game::play)]
#[test]
fn accepted_move_advances_turn_and_rejected_moves_preserve_position() {
    let mut game = Game::new();
    game.play(Player::Black, "7g7f".parse().unwrap()).unwrap();
    assert_eq!(game.turn(), Player::White);
    let before = game.position();
    assert_eq!(
        game.play(Player::Black, "2g2f".parse().unwrap()),
        Err(CommandError::WrongTurn)
    );
    assert_eq!(game.position(), before);
    assert_eq!(
        game.play(Player::White, "3c3e".parse().unwrap()),
        Err(CommandError::IllegalMove)
    );
    assert_eq!(game.position(), before);
    assert_eq!(game.turn(), Player::White);
    assert!(game.result().is_none());
}

#[traceability::targets(shogi_domain::Game::resign, shogi_domain::Game::play)]
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
    assert_eq!(
        game.play(Player::Black, "7g7f".parse().unwrap()),
        Err(CommandError::GameFinished)
    );
    assert_eq!(game.position(), before);
}

#[test]
fn timeout_retains_position_and_finishes_against_side_to_move() {
    let mut game = Game::new();
    let before = game.position();
    let result = game.timeout().unwrap();
    assert_eq!(result.reason, shogi_domain::FinishReason::Timeout);
    assert_eq!(result.winner, Player::White);
    assert_eq!(game.position(), before);
    assert!(game.legal_moves().is_empty());
    assert_eq!(game.timeout(), Err(CommandError::GameFinished));
}
