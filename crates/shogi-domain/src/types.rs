use std::{error::Error, fmt};

/// Black moves first and advances toward rank 1. White advances toward rank 9.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Player {
    /// The first player, advancing toward rank 1.
    Black,
    /// The second player, advancing toward rank 9.
    White,
}

impl Player {
    /// Returns the other side.
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

/// An unpromoted captured piece that can be held in hand and dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HandPiece {
    /// An unpromoted pawn.
    Pawn,
    /// An unpromoted lance.
    Lance,
    /// An unpromoted knight.
    Knight,
    /// An unpromoted silver general.
    Silver,
    /// A gold general.
    Gold,
    /// An unpromoted bishop.
    Bishop,
    /// An unpromoted rook.
    Rook,
}

impl HandPiece {
    /// All seven hand pieces, ordered pawn, lance, knight, silver, gold, bishop, rook.
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

/// A board piece, including promoted forms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PieceKind {
    /// An unpromoted pawn.
    Pawn,
    /// An unpromoted lance.
    Lance,
    /// An unpromoted knight.
    Knight,
    /// An unpromoted silver general.
    Silver,
    /// A gold general.
    Gold,
    /// An unpromoted bishop.
    Bishop,
    /// An unpromoted rook.
    Rook,
    /// The king, which cannot be captured into hand.
    King,
    /// A promoted pawn (tokin), moving as a gold.
    PromotedPawn,
    /// A promoted lance, moving as a gold.
    PromotedLance,
    /// A promoted knight, moving as a gold.
    PromotedKnight,
    /// A promoted silver, moving as a gold.
    PromotedSilver,
    /// A promoted bishop.
    Horse,
    /// A promoted rook.
    Dragon,
}

/// A piece on the board with its current owner and promotion state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Piece {
    /// The side that currently owns this piece.
    pub owner: Player,
    /// The current piece kind, including promotion.
    pub kind: PieceKind,
}

/// A square in absolute coordinates. Files and ranks are both 1 through 9.
/// Rank 1 is USI `a`, rank 9 is USI `i`, for either player.
/// Parsing and formatting use exact USI coordinates such as `7g`.
/// Parsing rejects whitespace, extra bytes, and out-of-range coordinates with
/// [`crate::ParseUsiError`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Square {
    file: u8,
    rank: u8,
}

impl Square {
    /// Constructs a square in absolute coordinates.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidSquare`] if either coordinate is outside `1..=9`.
    pub const fn new(file: u8, rank: u8) -> Result<Self, InvalidSquare> {
        if file >= 1 && file <= 9 && rank >= 1 && rank <= 9 {
            Ok(Self { file, rank })
        } else {
            Err(InvalidSquare)
        }
    }

    /// Returns the file in `1..=9`, as used by USI.
    pub const fn file(self) -> u8 {
        self.file
    }
    /// Returns the absolute rank in `1..=9` (`a` through `i` in USI).
    pub const fn rank(self) -> u8 {
        self.rank
    }

    pub(crate) const fn index(self) -> usize {
        (self.file as usize - 1) * 9 + self.rank as usize - 1
    }
}

/// A file or rank lies outside `1..=9`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidSquare;

impl fmt::Display for InvalidSquare {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("square file and rank must be in 1..=9")
    }
}
impl Error for InvalidSquare {}

/// A board move or drop, without a claim of position legality.
///
/// Parsing and formatting use exact USI syntax, with no whitespace. Board moves
/// use `7g7f` or `2b3c+`; drops use uppercase `PLNSGBR`, such as `P*5e`.
/// Promoted pieces and kings cannot be dropped. Parsing does not check the board.
///
/// ```
/// use shogi_domain::{HandPiece, Move, Square};
///
/// assert_eq!("7g".parse::<Square>()?, Square::new(7, 7)?);
/// let movement: Move = "P*5e".parse()?;
/// assert_eq!(movement, Move::Drop { piece: HandPiece::Pawn, to: Square::new(5, 5)? });
/// assert_eq!(movement.to_string(), "P*5e");
/// assert!(" P*5e".parse::<Move>().is_err());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Move {
    /// Moves an existing board piece, optionally promoting it.
    Board {
        /// The occupied starting square.
        from: Square,
        /// The destination square.
        to: Square,
        /// Whether this move requests promotion.
        promote: bool,
    },
    /// Places a piece from the moving player's hand onto the board.
    Drop {
        /// The unpromoted hand piece to drop.
        piece: HandPiece,
        /// The destination square.
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
    /// Returns the occupying piece, or `None` for an empty square.
    pub fn piece_at(&self, square: Square) -> Option<Piece> {
        self.board[square.index()]
    }

    /// Returns how many pieces of this kind the player holds in hand.
    pub fn hand_count(&self, player: Player, piece: HandPiece) -> u32 {
        self.hands[player.index()][piece.index()]
    }
}

/// A terminal condition adjudicated by this crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinishReason {
    /// The side to move is checked and has no legal moves.
    Checkmate,
    /// A player resigned, including possibly off turn.
    Resignation,
}

/// The winner and the condition that ended the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameResult {
    /// The winning side.
    pub winner: Player,
    /// The terminal condition.
    pub reason: FinishReason,
}

/// Check status and result of the position after an accepted move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveOutcome {
    /// Whether the side to move after the command is checked.
    pub in_check: bool,
    /// The terminal result, or `None` if play continues.
    pub result: Option<GameResult>,
}

/// A rejected command. The game remains unchanged for every variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandError {
    /// The game already has a terminal result.
    GameFinished,
    /// The supplied player does not have the turn.
    WrongTurn,
    /// The move is not legal in the current position.
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
