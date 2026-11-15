crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct shogi-domain fixture and observable game state; no persistence, authentication or network transport.",
        reason: "Mutating a game does not change a previously returned position.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}
