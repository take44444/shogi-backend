use crate::{CommandError, HandPiece, Move, Piece, PieceKind, Player, PositionView, Square};
use rsshogi::{
    board::{self, Move32List, Position},
    movegen::generate_legal_all_move32,
    types::{Color, HandPiece as RulesHandPiece, Move32, PieceType, Square as RulesSquare},
};

#[derive(Clone)]
pub(crate) struct Rules {
    position: Position,
}

impl Rules {
    pub(crate) fn new() -> Self {
        board::init();
        Self {
            position: Position::from_sfen(
                "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
            )
            .expect("standard initial position"),
        }
    }

    pub(crate) fn turn(&self) -> Player {
        player(self.position.turn())
    }
    pub(crate) fn in_check(&self) -> bool {
        self.position.is_in_check()
    }

    fn generated_moves(&self) -> Move32List {
        let mut moves = Move32List::new();
        generate_legal_all_move32(&self.position, &mut moves);
        moves
    }

    pub(crate) fn legal_moves(&self) -> Vec<Move> {
        self.generated_moves()
            .iter()
            .copied()
            .map(domain_move)
            .collect()
    }

    pub(crate) fn play(&mut self, mv: Move) -> Result<(), CommandError> {
        let generated = self.generated_moves();
        let legal = generated
            .iter()
            .copied()
            .find(|candidate| domain_move(*candidate) == mv)
            .ok_or(CommandError::IllegalMove)?;
        // The fallible update guarantees no mutation on error, including counter overflow.
        self.position
            .try_apply_move32(legal)
            .map_err(|_| CommandError::RulesFailure)
    }

    pub(crate) fn position(&self) -> PositionView {
        let board = std::array::from_fn(|i| {
            let piece = self.position.piece_on(RulesSquare::from_index(i));
            if piece.is_empty() {
                None
            } else {
                Some(Piece {
                    owner: player(piece.color()),
                    kind: piece_kind(piece.piece_type()),
                })
            }
        });
        let hands = [Color::BLACK, Color::WHITE].map(|color| {
            HandPiece::ALL.map(|piece| self.position.hand(color).count(hand_piece(piece)))
        });
        PositionView { board, hands }
    }

    #[cfg(test)]
    pub(crate) fn from_sfen(sfen: &str) -> Self {
        board::init();
        Self {
            position: Position::from_sfen(sfen).expect("valid test fixture"),
        }
    }
}

fn player(color: Color) -> Player {
    match color {
        Color::BLACK => Player::Black,
        Color::WHITE => Player::White,
    }
}

fn square(square: RulesSquare) -> Square {
    Square::new(square.file().raw() as u8 + 1, square.rank().raw() as u8 + 1)
        .expect("generated squares are on the board")
}

fn domain_move(mv: Move32) -> Move {
    let to = square(mv.to_sq());
    if let Some(piece) = mv.dropped_piece() {
        let piece = match piece {
            PieceType::PAWN => HandPiece::Pawn,
            PieceType::LANCE => HandPiece::Lance,
            PieceType::KNIGHT => HandPiece::Knight,
            PieceType::SILVER => HandPiece::Silver,
            PieceType::GOLD => HandPiece::Gold,
            PieceType::BISHOP => HandPiece::Bishop,
            PieceType::ROOK => HandPiece::Rook,
            _ => unreachable!("generated drops contain hand pieces"),
        };
        Move::Drop { piece, to }
    } else {
        Move::Board {
            from: square(mv.from_sq()),
            to,
            promote: mv.is_promotion(),
        }
    }
}

fn hand_piece(piece: HandPiece) -> RulesHandPiece {
    match piece {
        HandPiece::Pawn => RulesHandPiece::PAWN,
        HandPiece::Lance => RulesHandPiece::LANCE,
        HandPiece::Knight => RulesHandPiece::KNIGHT,
        HandPiece::Silver => RulesHandPiece::SILVER,
        HandPiece::Gold => RulesHandPiece::GOLD,
        HandPiece::Bishop => RulesHandPiece::BISHOP,
        HandPiece::Rook => RulesHandPiece::ROOK,
    }
}

fn piece_kind(piece: PieceType) -> PieceKind {
    match piece {
        PieceType::PAWN => PieceKind::Pawn,
        PieceType::LANCE => PieceKind::Lance,
        PieceType::KNIGHT => PieceKind::Knight,
        PieceType::SILVER => PieceKind::Silver,
        PieceType::GOLD => PieceKind::Gold,
        PieceType::BISHOP => PieceKind::Bishop,
        PieceType::ROOK => PieceKind::Rook,
        PieceType::KING => PieceKind::King,
        PieceType::PRO_PAWN => PieceKind::PromotedPawn,
        PieceType::PRO_LANCE => PieceKind::PromotedLance,
        PieceType::PRO_KNIGHT => PieceKind::PromotedKnight,
        PieceType::PRO_SILVER => PieceKind::PromotedSilver,
        PieceType::HORSE => PieceKind::Horse,
        PieceType::DRAGON => PieceKind::Dragon,
        _ => unreachable!("position pieces have an actual piece kind"),
    }
}
