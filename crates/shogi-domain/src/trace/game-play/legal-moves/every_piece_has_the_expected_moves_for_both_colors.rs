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
