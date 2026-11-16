use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Every ordinary/promoted piece movement is compared for both colors.",
        reason: "Every ordinary/promoted piece movement is compared for both colors.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Sliders stop at friendly pieces and cannot move beyond captured enemies.",
        reason: "Sliders stop at friendly pieces and cannot move beyond captured enemies.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Captures demote every promoted kind into hand and a drop consumes a held piece.",
        reason: "Captures demote every promoted kind into hand and a drop consumes a held piece.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Optional nonpromotions remain legal and dead-rank moves require promotion.",
        reason: "Optional nonpromotions remain legal and dead-rank moves require promotion.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Nifu, promoted-pawn exception, occupied-square and unavailable-hand drop cases are asserted.",
        reason: "Nifu, promoted-pawn exception, occupied-square and unavailable-hand drop cases are asserted.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Pawn/lance/knight dead-rank drops are rejected for both sides.",
        reason: "Pawn/lance/knight dead-rank drops are rejected for both sides.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Pins, legal check evasions and double-check legal moves are asserted.",
        reason: "Pins, legal check evasions and double-check legal moves are asserted.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Pawn-drop mate is rejected while a checking pawn drop with an escape remains legal.",
        reason: "Pawn-drop mate is rejected while a checking pawn drop with an escape remains legal.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. RulesFailure is distinct and preserves state on a rules-engine counter failure.",
        reason: "RulesFailure is distinct and preserves state on a rules-engine counter failure.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
fn counter_failure_is_distinct_and_atomic() {
    let mut game = Game::from_sfen("4k4/9/9/9/9/9/9/9/4K4 b - 65535");
    assert!(game.legal_moves().contains(&mv("5i4i")));
    reject(&mut game, Player::Black, "5i4i", CommandError::RulesFailure);
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. An ordinary advancing pawn may legally checkmate.",
        reason: "An ordinary advancing pawn may legally checkmate.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport. Promotion is allowed when a move starts in the zone and exits it.",
        reason: "Promotion is allowed when a move starts in the zone and exits it.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}
