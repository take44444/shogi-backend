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

include!(
    "trace/game-snapshots-access/owned-position/initial_position_and_snapshot_independence.rs"
);

include!("trace/game-play/notation/square_bounds_and_notation_are_strict.rs");

include!("trace/game-play/domain-play/rejection_preserves_all_observable_state.rs");

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

include!("trace/game-play/legal-moves/every_piece_has_the_expected_moves_for_both_colors.rs");

include!("trace/game-play/legal-moves/sliders_stop_at_friendly_and_enemy_blockers.rs");

include!(
    "trace/game-play/legal-moves/capture_demotes_every_promoted_kind_and_drop_consumes_hand.rs"
);

include!(
    "trace/game-play/legal-moves/promotions_include_optional_nonpromotions_and_require_dead_rank_promotion.rs"
);

include!("trace/game-play/legal-moves/nifu_promoted_pawn_exception_occupied_and_missing_drops.rs");

include!("trace/game-play/legal-moves/dead_rank_drops_for_both_players.rs");

include!("trace/game-play/legal-moves/king_safety_pins_check_evasions_and_double_check.rs");

include!(
    "trace/game-play/legal-moves/pawn_drop_mate_is_rejected_but_checking_pawn_with_escape_is_allowed.rs"
);

include!(
    "trace/game-play/domain-play/checkmate_finishes_and_finished_precedes_turn_or_legality.rs"
);

include!("trace/game-play/domain-play/no_legal_moves_without_check_is_not_checkmate.rs");

include!("trace/game-play/domain-resign/either_player_can_resign_on_or_off_turn.rs");

include!("trace/game-play/domain-play/repeated_positions_and_perpetual_check_remain_playable.rs");

include!("trace/game-play/legal-moves/counter_failure_is_distinct_and_atomic.rs");

include!("trace/game-play/domain-play/public_errors_implement_standard_error.rs");

include!("trace/game-play/legal-moves/ordinary_pawn_advance_mate_is_accepted.rs");

include!("trace/game-play/legal-moves/promotion_can_start_in_zone_and_end_outside_it.rs");

include!(
    "trace/game-snapshots-access/owned-position/cloned_games_own_their_position_and_terminal_result.rs"
);
