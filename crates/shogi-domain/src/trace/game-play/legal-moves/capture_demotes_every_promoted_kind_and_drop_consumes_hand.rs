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
