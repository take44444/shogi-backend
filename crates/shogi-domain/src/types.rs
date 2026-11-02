use std::{error::Error, fmt};

/// Black moves first and advances toward rank 1. White advances toward rank 9.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Player {
    Black,
    White,
}

impl Player {
    pub const fn opponent(self) -> Self {
        match self {
            Self::Black => Self::White,
            Self::White => Self::Black,
        }
    }

    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Black => 0,
            Self::White => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HandPiece {
    Pawn,
    Lance,
    Knight,
    Silver,
    Gold,
    Bishop,
    Rook,
}

impl HandPiece {
    pub const ALL: [Self; 7] = [
        Self::Pawn,
        Self::Lance,
        Self::Knight,
        Self::Silver,
        Self::Gold,
        Self::Bishop,
        Self::Rook,
    ];

    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Pawn => 0,
            Self::Lance => 1,
            Self::Knight => 2,
            Self::Silver => 3,
            Self::Gold => 4,
            Self::Bishop => 5,
            Self::Rook => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PieceKind {
    Pawn,
    Lance,
    Knight,
    Silver,
    Gold,
    Bishop,
    Rook,
    King,
    PromotedPawn,
    PromotedLance,
    PromotedKnight,
    PromotedSilver,
    Horse,
    Dragon,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Piece {
    pub owner: Player,
    pub kind: PieceKind,
}

/// A square in absolute coordinates. Files and ranks are both 1 through 9.
/// Rank 1 is USI `a`, rank 9 is USI `i`, for either player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Square {
    file: u8,
    rank: u8,
}

impl Square {
    pub const fn new(file: u8, rank: u8) -> Result<Self, InvalidSquare> {
        if file >= 1 && file <= 9 && rank >= 1 && rank <= 9 {
            Ok(Self { file, rank })
        } else {
            Err(InvalidSquare)
        }
    }

    pub const fn file(self) -> u8 {
        self.file
    }
    pub const fn rank(self) -> u8 {
        self.rank
    }

    pub(crate) const fn index(self) -> usize {
        (self.file as usize - 1) * 9 + self.rank as usize - 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidSquare;

impl fmt::Display for InvalidSquare {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("square file and rank must be in 1..=9")
    }
}
impl Error for InvalidSquare {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Move {
    Board {
        from: Square,
        to: Square,
        promote: bool,
    },
    Drop {
        piece: HandPiece,
        to: Square,
    },
}

/// An owned, read-only snapshot. Accessors never expose mutable game state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PositionView {
    pub(crate) board: [Option<Piece>; 81],
    pub(crate) hands: [[u32; 7]; 2],
}

impl PositionView {
    pub fn piece_at(&self, square: Square) -> Option<Piece> {
        self.board[square.index()]
    }

    pub fn hand_count(&self, player: Player, piece: HandPiece) -> u32 {
        self.hands[player.index()][piece.index()]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinishReason {
    Checkmate,
    Resignation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameResult {
    pub winner: Player,
    pub reason: FinishReason,
}

/// Check status and result of the position after an accepted move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveOutcome {
    pub in_check: bool,
    pub result: Option<GameResult>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandError {
    GameFinished,
    WrongTurn,
    IllegalMove,
    /// The rules implementation could not apply a legal command.
    RulesFailure,
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::GameFinished => "the game has finished",
            Self::WrongTurn => "the player does not have the turn",
            Self::IllegalMove => "the move is illegal in this position",
            Self::RulesFailure => "the rules implementation could not apply the move",
        })
    }
}
impl Error for CommandError {}
