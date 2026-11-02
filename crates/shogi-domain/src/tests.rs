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

#[test]
fn initial_position_and_snapshot_independence() {
    let mut game = Game::default();
    assert_eq!(game.turn(), Player::Black);
    assert!(!game.in_check());
    assert_eq!(game.result(), None);
    assert_eq!(game.legal_moves().len(), 30);
    let initial = game.position();
    for file in 1..=9 {
        for (rank, owner) in [(3, Player::White), (7, Player::Black)] {
            assert_eq!(
                initial.piece_at(Square::new(file, rank).unwrap()),
                Some(Piece {
                    owner,
                    kind: PieceKind::Pawn
                })
            );
        }
    }
    for player in [Player::Black, Player::White] {
        for piece in HandPiece::ALL {
            assert_eq!(initial.hand_count(player, piece), 0);
        }
    }
    assert_eq!(initial.piece_at(sq("8h")).unwrap().kind, PieceKind::Bishop);
    assert_eq!(initial.piece_at(sq("2h")).unwrap().kind, PieceKind::Rook);
    play(&mut game, "7g7f");
    assert!(game.position().piece_at(sq("7g")).is_none());
    assert!(initial.piece_at(sq("7g")).is_some());
}

#[test]
fn square_bounds_and_notation_are_strict() {
    for file in 0..=u8::MAX {
        for rank in 0..=u8::MAX {
            assert_eq!(
                Square::new(file, rank).is_ok(),
                (1..=9).contains(&file) && (1..=9).contains(&rank)
            );
        }
    }
    for file in 1..=9 {
        for rank in 1..=9 {
            let square = Square::new(file, rank).unwrap();
            assert_eq!(square.file(), file);
            assert_eq!(square.rank(), rank);
            assert_eq!(square.to_string().parse::<Square>(), Ok(square));
        }
    }
    for valid in [
        "7g7f", "2b3c+", "P*5e", "L*1b", "N*1c", "S*3a", "G*4i", "B*8h", "R*2b",
    ] {
        assert_eq!(mv(valid).to_string(), valid);
    }
    for invalid in [
        "", " 7g7f", "7g7f\n", "7g7f++", "7g7f-", "7G7f", "0g7f", "7j7f", "7g0f", "7g7j", "P*5e+",
        "p*5e", "K*5e", "+P*5e", "resign", "win", "0000", "7g7f\0", "é*5e", "７g7f", "7g7🦀",
    ] {
        assert!(invalid.parse::<Move>().is_err(), "{invalid:?}");
    }
    for invalid in ["", "1A", "0a", "1j", "1aa", " 1a", "１a", "🦀"] {
        assert!(invalid.parse::<Square>().is_err());
    }
}

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

#[test]
fn every_piece_has_the_expected_moves_for_both_colors() {
    let gold = vec![(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (0, 1)];
    let king = vec![
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];
    let diagonal: Vec<_> = (-4i8..=4)
        .filter(|n| *n != 0)
        .flat_map(|n| [(n, n), (n, -n)])
        .collect();
    let straight: Vec<_> = (-4i8..=4)
        .filter(|n| *n != 0)
        .flat_map(|n| [(n, 0), (0, n)])
        .collect();
    let mut horse = diagonal.clone();
    horse.extend([(-1, 0), (1, 0), (0, -1), (0, 1)]);
    let mut dragon = straight.clone();
    dragon.extend([(-1, -1), (-1, 1), (1, -1), (1, 1)]);
    for (piece, offsets) in [
        ("P", vec![(0, -1)]),
        ("L", vec![(0, -1), (0, -2), (0, -3)]),
        ("N", vec![(-1, -2), (1, -2)]),
        ("S", vec![(-1, -1), (0, -1), (1, -1), (-1, 1), (1, 1)]),
        ("G", gold.clone()),
        ("B", diagonal),
        ("R", straight),
        ("K", king),
        ("+P", gold.clone()),
        ("+L", gold.clone()),
        ("+N", gold.clone()),
        ("+S", gold),
        ("+B", horse),
        ("+R", dragon),
    ] {
        for player in [Player::Black, Player::White] {
            let mut rows = vec![vec![String::new(); 9]; 9];
            // Kings on off-diagonal corners avoid interfering with center sliders.
            rows[0][2] = "k".into();
            rows[8][6] = "K".into();
            if piece == "K" {
                rows[if player == Player::Black { 8 } else { 0 }]
                    [if player == Player::Black { 6 } else { 2 }]
                .clear();
            }
            rows[4][4] = if player == Player::Black {
                piece.into()
            } else {
                piece.to_lowercase()
            };
            let mut game = game_from_rows(rows, player, "-");
            for file in 1..=9 {
                for rank in 1..=9 {
                    let from = sq("5e");
                    let to = Square::new(file, rank).unwrap();
                    let df = file as i8 - 5;
                    let dr = (rank as i8 - 5) * if player == Player::Black { 1 } else { -1 };
                    let movement = Move::Board {
                        from,
                        to,
                        promote: false,
                    };
                    let expected = offsets.contains(&(df, dr));
                    assert_eq!(
                        game.legal_moves().contains(&movement),
                        expected,
                        "{player:?} {piece} -> {to}"
                    );
                }
            }
            let movement = game
                .legal_moves()
                .into_iter()
                .find(|m| matches!(m, Move::Board {from, ..} if *from == sq("5e")))
                .unwrap();
            game.play(player, movement).unwrap();
        }
    }
}

#[test]
fn sliders_stop_at_friendly_and_enemy_blockers() {
    for (sfen, from, legal, illegal) in [
        ("k8/9/9/4P4/4R4/9/9/9/8K b - 1", "5e", "5e5f", "5e5b"),
        ("k8/9/9/4p4/4R4/9/9/9/8K b - 1", "5e", "5e5d", "5e5c"),
        ("k8/9/2P6/9/4B4/9/9/9/8K b - 1", "5e", "5e6d", "5e8b"),
        ("k8/9/9/4P4/4L4/9/9/9/8K b - 1", "5e", "5e5e", "5e5c"),
    ] {
        let mut game = Game::from_sfen(sfen);
        assert!(game.position().piece_at(sq(from)).is_some());
        if legal != "5e5e" {
            assert!(game.legal_moves().contains(&mv(legal)));
        }
        reject(&mut game, Player::Black, illegal, CommandError::IllegalMove);
    }
}

#[test]
fn capture_demotes_every_promoted_kind_and_drop_consumes_hand() {
    for (captured, hand) in [
        ("+p", HandPiece::Pawn),
        ("+l", HandPiece::Lance),
        ("+n", HandPiece::Knight),
        ("+s", HandPiece::Silver),
        ("+b", HandPiece::Bishop),
        ("+r", HandPiece::Rook),
        ("g", HandPiece::Gold),
    ] {
        let mut game = Game::from_sfen(&format!("k8/9/9/4{captured}4/4R4/9/9/9/8K b - 1"));
        play(&mut game, "5e5d");
        assert_eq!(game.position().hand_count(Player::Black, hand), 1);
        assert_eq!(
            game.position().piece_at(sq("5d")),
            Some(Piece {
                owner: Player::Black,
                kind: PieceKind::Rook
            })
        );
        play(&mut game, "9a8a");
        let drop = Move::Drop {
            piece: hand,
            to: sq("4e"),
        };
        game.play(Player::Black, drop).unwrap();
        assert_eq!(game.position().hand_count(Player::Black, hand), 0);
        assert_eq!(
            game.position().piece_at(sq("4e")).unwrap().owner,
            Player::Black
        );
        play(&mut game, "8a9a");
        let before = snapshot(&game);
        assert_eq!(
            game.play(
                Player::Black,
                Move::Drop {
                    piece: hand,
                    to: sq("3e")
                }
            ),
            Err(CommandError::IllegalMove)
        );
        assert_eq!(snapshot(&game), before);
    }
}

#[test]
fn promotions_include_optional_nonpromotions_and_require_dead_rank_promotion() {
    for player in [Player::Black, Player::White] {
        for (piece, start, end) in [("P", 2, 1), ("L", 2, 1), ("N", 3, 1), ("N", 4, 2)] {
            let (start, end) = if player == Player::Black {
                (start, end)
            } else {
                (10 - start, 10 - end)
            };
            let from = Square::new(5, start).unwrap();
            let to = Square::new(if piece == "N" { 4 } else { 5 }, end).unwrap();
            let mut game = fixture(piece, player, 5, start);
            let plain = Move::Board {
                from,
                to,
                promote: false,
            };
            assert!(!game.legal_moves().contains(&plain));
            let before = snapshot(&game);
            assert_eq!(game.play(player, plain), Err(CommandError::IllegalMove));
            assert_eq!(snapshot(&game), before);
            game.play(
                player,
                Move::Board {
                    from,
                    to,
                    promote: true,
                },
            )
            .unwrap();
        }
        for piece in ["P", "L", "N", "S", "B", "R"] {
            let start = if piece == "N" {
                5
            } else if player == Player::Black {
                4
            } else {
                6
            };
            let (file, end) = match piece {
                "N" => (4, if player == Player::Black { 3 } else { 7 }),
                "B" => (4, if player == Player::Black { 3 } else { 7 }),
                _ => (5, if player == Player::Black { 3 } else { 7 }),
            };
            let game = fixture(piece, player, 5, start);
            for promote in [false, true] {
                assert!(
                    game.legal_moves().contains(&Move::Board {
                        from: Square::new(5, start).unwrap(),
                        to: Square::new(file, end).unwrap(),
                        promote
                    }),
                    "{player:?} {piece} optional {promote}"
                );
            }
        }
    }
    for piece in [
        "P", "L", "N", "S", "B", "R", "G", "K", "+P", "+L", "+N", "+S", "+B", "+R",
    ] {
        let mut game = fixture(piece, Player::Black, 5, 5);
        reject(&mut game, Player::Black, "5e5d+", CommandError::IllegalMove);
    }
    for piece in ["G", "K", "+P", "+L", "+N", "+S", "+B", "+R"] {
        let mut game = fixture(piece, Player::Black, 5, 3);
        reject(&mut game, Player::Black, "5c5b+", CommandError::IllegalMove);
    }
}

#[test]
fn nifu_promoted_pawn_exception_occupied_and_missing_drops() {
    for (sfen, player, nifu, allowed, occupied) in [
        (
            "k8/9/9/9/4P4/9/9/9/8K b P 1",
            Player::Black,
            "P*5d",
            "P*4d",
            "P*5e",
        ),
        (
            "k8/9/9/9/4p4/9/9/9/8K w p 1",
            Player::White,
            "P*5f",
            "P*4f",
            "P*5e",
        ),
    ] {
        let mut game = Game::from_sfen(sfen);
        reject(&mut game, player, nifu, CommandError::IllegalMove);
        reject(&mut game, player, occupied, CommandError::IllegalMove);
        reject(&mut game, player, "R*4e", CommandError::IllegalMove);
        play(&mut game, allowed);
    }
    for (sfen, movement) in [
        ("k8/9/9/9/4+P4/9/9/9/8K b P 1", "P*5d"),
        ("k8/9/9/9/4+p4/9/9/9/8K w p 1", "P*5f"),
    ] {
        play(&mut Game::from_sfen(sfen), movement);
    }
}

#[test]
fn dead_rank_drops_for_both_players() {
    for player in [Player::Black, Player::White] {
        let mut game = Game::from_sfen(if player == Player::Black {
            "k8/9/9/9/9/9/9/9/8K b PLN 1"
        } else {
            "k8/9/9/9/9/9/9/9/8K w pln 1"
        });
        for (symbol, dead) in [("P", vec![1]), ("L", vec![1]), ("N", vec![1, 2])] {
            for rank in dead {
                let rank = if player == Player::Black {
                    rank
                } else {
                    10 - rank
                };
                reject(
                    &mut game,
                    player,
                    &format!("{symbol}*{}", Square::new(5, rank).unwrap()),
                    CommandError::IllegalMove,
                );
            }
        }
        for piece in [HandPiece::Pawn, HandPiece::Lance, HandPiece::Knight] {
            assert!(game.legal_moves().contains(&Move::Drop {
                piece,
                to: Square::new(5, if player == Player::Black { 3 } else { 7 }).unwrap()
            }));
        }
    }
}

#[test]
fn king_safety_pins_check_evasions_and_double_check() {
    let mut pinned = Game::from_sfen("k3r4/9/9/9/9/9/9/4G4/4K4 b - 1");
    assert!(!pinned.in_check());
    reject(
        &mut pinned,
        Player::Black,
        "5h4h",
        CommandError::IllegalMove,
    );
    assert!(pinned.legal_moves().contains(&mv("5h5g")));
    let mut checked = Game::from_sfen("k3r4/9/9/9/9/9/9/9/4K4 b GS 1");
    assert!(checked.in_check());
    reject(
        &mut checked,
        Player::Black,
        "S*4h",
        CommandError::IllegalMove,
    );
    reject(
        &mut checked,
        Player::Black,
        "5i5h",
        CommandError::IllegalMove,
    );
    play(&mut checked, "G*5h");
    assert!(!checked.in_check());
    let mut double = Game::from_sfen("k3r4/9/9/9/b8/9/9/9/4K4 b G 1");
    assert!(double.in_check());
    reject(
        &mut double,
        Player::Black,
        "G*5h",
        CommandError::IllegalMove,
    );
    assert!(
        double
            .legal_moves()
            .iter()
            .all(|m| matches!(m,Move::Board {from,..} if *from==sq("5i")))
    );
}

#[test]
fn pawn_drop_mate_is_rejected_but_checking_pawn_with_escape_is_allowed() {
    let mut game = Game::from_sfen("3pkp3/3p1p3/4G4/9/9/9/9/9/K3R4 b P 1");
    reject(&mut game, Player::Black, "P*5b", CommandError::IllegalMove);
    let mut escape = Game::from_sfen("3pk4/3p1p3/4G4/9/9/9/9/9/K3R4 b P 1");
    let outcome = play(&mut escape, "P*5b");
    assert!(outcome.in_check);
    assert_eq!(outcome.result, None);
    assert!(escape.legal_moves().contains(&mv("5a4a")));
    play(&mut escape, "5a4a");
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
fn counter_failure_is_distinct_and_atomic() {
    let mut game = Game::from_sfen("4k4/9/9/9/9/9/9/9/4K4 b - 65535");
    assert!(game.legal_moves().contains(&mv("5i4i")));
    reject(&mut game, Player::Black, "5i4i", CommandError::RulesFailure);
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

#[test]
fn ordinary_pawn_advance_mate_is_accepted() {
    let mut game = Game::from_sfen("3pkp3/3p1p3/4P4/9/9/9/9/9/K3R4 b - 1");
    let outcome = play(&mut game, "5c5b");
    assert!(outcome.in_check);
    assert_eq!(
        outcome.result,
        Some(GameResult {
            winner: Player::Black,
            reason: FinishReason::Checkmate
        })
    );
}

#[test]
fn promotion_can_start_in_zone_and_end_outside_it() {
    for (sfen, movement, kind) in [
        (
            "k8/9/4S4/9/9/9/9/9/8K b - 1",
            "5c4d+",
            PieceKind::PromotedSilver,
        ),
        (
            "k8/9/9/9/9/9/4s4/9/8K w - 1",
            "5g4f+",
            PieceKind::PromotedSilver,
        ),
        ("k8/9/4B4/9/9/9/9/9/8K b - 1", "5c4d+", PieceKind::Horse),
        ("k8/9/9/9/9/9/4r4/9/8K w - 1", "5g5f+", PieceKind::Dragon),
    ] {
        let mut game = Game::from_sfen(sfen);
        let plain = &movement[..4];
        assert!(game.legal_moves().contains(&mv(plain)));
        play(&mut game, movement);
        assert_eq!(
            game.position().piece_at(sq(&movement[2..4])).unwrap().kind,
            kind
        );
    }
}

#[test]
fn cloned_games_own_their_position_and_terminal_result() {
    let original = Game::new();
    let initial = original.position();
    let mut staged = original.clone();
    staged.play(Player::Black, "7g7f".parse().unwrap()).unwrap();
    assert_eq!(original.position(), initial);
    assert_ne!(staged.position(), initial);
    staged.resign(Player::Black).unwrap();
    assert_eq!(original.result(), None);
    let finished = staged.clone();
    assert_eq!(finished.result(), staged.result());
    assert_eq!(finished.position(), staged.position());
    assert!(finished.legal_moves().is_empty());
}
