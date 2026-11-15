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
