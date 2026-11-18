use super::*;

#[test]
fn either_player_can_resign_on_or_off_turn() {
    for move_first in [false, true] {
        for player in [Player::Black, Player::White] {
            let mut game = Game::new();
            if move_first {
                play(&mut game, "7g7f");
            }
            let position = game.position();
            let turn = game.turn();
            let check = game.in_check();
            let result = GameResult {
                winner: player.opponent(),
                reason: FinishReason::Resignation,
            };
            assert_eq!(game.resign(player), Ok(result));
            assert_eq!(game.result(), Some(result));
            assert_eq!(game.position(), position);
            assert_eq!(game.turn(), turn);
            assert_eq!(game.in_check(), check);
            assert!(game.legal_moves().is_empty());
            reject(&mut game, player, "7g7f", CommandError::GameFinished);
            let before = snapshot(&game);
            assert_eq!(
                game.resign(player.opponent()),
                Err(CommandError::GameFinished)
            );
            assert_eq!(snapshot(&game), before);
        }
    }
}
