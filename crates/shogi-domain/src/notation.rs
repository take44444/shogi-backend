use crate::{HandPiece, Move, Square};
use std::{error::Error, fmt, str::FromStr};

/// Input is not an exact USI square, board move, or unpromoted piece drop.
/// Whitespace, invalid coordinates, lowercase drop symbols, and extra bytes fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseUsiError;

impl fmt::Display for ParseUsiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("expected a USI square or board move or unpromoted piece drop")
    }
}
impl Error for ParseUsiError {}

fn square(bytes: &[u8]) -> Result<Square, ParseUsiError> {
    match bytes {
        [file @ b'1'..=b'9', rank @ b'a'..=b'i'] => {
            Square::new(file - b'0', rank - b'a' + 1).map_err(|_| ParseUsiError)
        }
        _ => Err(ParseUsiError),
    }
}

impl FromStr for Square {
    type Err = ParseUsiError;

    /// Parses exactly one USI file digit and rank letter, such as `7g`.
    ///
    /// # Errors
    ///
    /// Returns [`ParseUsiError`] for anything outside `1a` through `9i`,
    /// including whitespace or extra bytes.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        square(s.as_bytes())
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.file(), char::from(b'a' + self.rank() - 1))
    }
}

impl FromStr for Move {
    type Err = ParseUsiError;

    /// Parses syntax only. Legality is checked by [`Game::play`](crate::Game::play).
    ///
    /// # Errors
    ///
    /// Returns [`ParseUsiError`] for invalid squares, malformed board moves,
    /// invalid drop symbols, whitespace, or extra bytes. Promotion uses a final
    /// `+`; drops use uppercase `PLNSGBR` followed by `*` and a square.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.as_bytes() {
            [piece, b'*', file, rank] => {
                let piece = match piece {
                    b'P' => HandPiece::Pawn,
                    b'L' => HandPiece::Lance,
                    b'N' => HandPiece::Knight,
                    b'S' => HandPiece::Silver,
                    b'G' => HandPiece::Gold,
                    b'B' => HandPiece::Bishop,
                    b'R' => HandPiece::Rook,
                    _ => return Err(ParseUsiError),
                };
                Ok(Self::Drop {
                    piece,
                    to: square(&[*file, *rank])?,
                })
            }
            [a, b, c, d] | [a, b, c, d, b'+'] => Ok(Self::Board {
                from: square(&[*a, *b])?,
                to: square(&[*c, *d])?,
                promote: s.len() == 5,
            }),
            _ => Err(ParseUsiError),
        }
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Board { from, to, promote } => {
                write!(f, "{from}{to}{}", if *promote { "+" } else { "" })
            }
            Self::Drop { piece, to } => {
                let symbol = match piece {
                    HandPiece::Pawn => 'P',
                    HandPiece::Lance => 'L',
                    HandPiece::Knight => 'N',
                    HandPiece::Silver => 'S',
                    HandPiece::Gold => 'G',
                    HandPiece::Bishop => 'B',
                    HandPiece::Rook => 'R',
                };
                write!(f, "{symbol}*{to}")
            }
        }
    }
}
