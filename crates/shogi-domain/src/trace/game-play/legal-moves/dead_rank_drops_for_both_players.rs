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
