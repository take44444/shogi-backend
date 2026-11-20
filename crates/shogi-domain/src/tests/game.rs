#![cfg(test)]

use super::*;

fn sq(s: &str) -> Square {
    s.parse().unwrap()
}
fn mv(s: &str) -> Move {
    s.parse().unwrap()
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    position: PositionView,
    turn: Player,
    check: bool,
    result: Option<GameResult>,
    legal: Vec<Move>,
}

fn snapshot(game: &Game) -> Snapshot {
    let mut legal = game.legal_moves();
    legal.sort_by_key(ToString::to_string);
    Snapshot {
        position: game.position(),
        turn: game.turn(),
        check: game.in_check(),
        result: game.result(),
        legal,
    }
}

fn reject(game: &mut Game, player: Player, movement: &str, error: CommandError) {
    let before = snapshot(game);
    assert_eq!(game.play(player, mv(movement)), Err(error), "{movement}");
    assert_eq!(snapshot(game), before, "rejected {movement} must be atomic");
}

fn play(game: &mut Game, movement: &str) -> MoveOutcome {
    game.play(game.turn(), mv(movement))
        .unwrap_or_else(|e| panic!("{movement}: {e}"))
}

#[path = "game_position.rs"]
mod game_snapshots_access_owned_position;

#[path = "game_notation.rs"]
mod game_play_notation;

#[path = "game_play.rs"]
mod game_play_domain_play;

fn fixture(piece: &str, player: Player, file: u8, rank: u8) -> Game {
    let mut rows = vec![vec![String::new(); 9]; 9];
    rows[0][0] = "k".into();
    rows[8][8] = "K".into();
    if piece == "K" {
        let (king_rank, king_file) = if player == Player::Black {
            (8, 8)
        } else {
            (0, 0)
        };
        rows[king_rank][king_file].clear();
    }
    rows[(rank - 1) as usize][(9 - file) as usize] = if player == Player::Black {
        piece.to_owned()
    } else {
        piece.to_lowercase()
    };
    game_from_rows(rows, player, "-")
}

fn game_from_rows(rows: Vec<Vec<String>>, player: Player, hand: &str) -> Game {
    let rows: Vec<_> = rows
        .into_iter()
        .map(|row| {
            let mut result = String::new();
            let mut empty = 0;
            for piece in row {
                if piece.is_empty() {
                    empty += 1;
                } else {
                    if empty > 0 {
                        result.push(char::from(b'0' + empty));
                        empty = 0;
                    }
                    result.push_str(&piece);
                }
            }
            if empty > 0 {
                result.push(char::from(b'0' + empty));
            }
            result
        })
        .collect();
    Game::from_sfen(&format!(
        "{} {} {hand} 1",
        rows.join("/"),
        if player == Player::Black { "b" } else { "w" }
    ))
}

#[path = "game_legal_moves.rs"]
mod game_play_legal_moves;

#[path = "game_resign.rs"]
mod game_play_domain_resign;
