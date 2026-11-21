use super::*;

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
