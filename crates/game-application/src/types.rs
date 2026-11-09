use std::{error::Error, fmt};

use shogi_domain::{CommandError, GameResult, Move, Player, PositionView};

pub use user_id::{InvalidId, UserId};

macro_rules! id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            /// Validates an ID without trimming or normalizing its contents.
            ///
            /// # Errors
            ///
            /// Returns [`InvalidId`] for zero or more than 128 UTF-8 bytes, or any
            /// character for which [`char::is_whitespace`] is true.
            pub fn new(value: impl Into<String>) -> Result<Self, InvalidId> {
                let value = value.into();
                if value.is_empty() || value.len() > 128 || value.chars().any(char::is_whitespace) {
                    return Err(InvalidId);
                }
                Ok(Self(value))
            }

            /// Borrows the original validated ID.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

id!(
    /// A game identifier containing 1 through 128 UTF-8 bytes and no Unicode whitespace.
    GameId
);
id!(
    /// A mutation request identifier, reserved on acceptance within one game.
    /// Contains 1 through 128 UTF-8 bytes and no Unicode whitespace.
    RequestId
);
/// Permanent side bindings. Game creation requires distinct user IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Participants {
    black: UserId,
    white: UserId,
}

impl Participants {
    /// Creates permanent bindings for distinct users.
    pub fn new(black: UserId, white: UserId) -> Result<Self, ApplicationError> {
        if black == white {
            return Err(ApplicationError::InvalidParticipants);
        }
        Ok(Self { black, white })
    }

    /// The user bound to Black.
    pub fn black(&self) -> &UserId {
        &self.black
    }

    /// The user bound to White.
    pub fn white(&self) -> &UserId {
        &self.white
    }

    /// Looks up a user's bound side, checking Black first.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::NonParticipant`] if neither binding matches.
    pub fn side(&self, user: &UserId) -> Result<Player, ApplicationError> {
        if user == &self.black {
            Ok(Player::Black)
        } else if user == &self.white {
            Ok(Player::White)
        } else {
            Err(ApplicationError::NonParticipant)
        }
    }
}

/// A typed move request. Exact retries must preserve every field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MakeMove {
    /// The request ID, reserved only after acceptance within this game.
    pub request_id: RequestId,
    /// The required pre-command sequence, starting at zero. Preserve it on retries.
    pub expected_seq: u64,
    /// The caller-supplied user claim, checked against the game's bindings.
    pub user_id: UserId,
    /// The typed move whose legality is checked against the current position.
    pub movement: Move,
}

/// A resignation request. Either participant may resign, including off turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resign {
    /// The request ID, reserved only after acceptance within this game.
    pub request_id: RequestId,
    /// The required pre-command sequence, starting at zero. Preserve it on retries.
    pub expected_seq: u64,
    /// The caller-supplied user claim, checked against the game's bindings.
    pub user_id: UserId,
}

/// The original outcome of an accepted command, retained for exact replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Acceptance {
    /// The sequence of this accepted command, in `1..=i64::MAX`. Replay retains it.
    pub seq: u64,
    /// Whether the side to move in the retained position is checked.
    pub in_check: bool,
    /// The terminal result, or `None` while the game continues.
    pub result: Option<GameResult>,
}

/// An owned view of a game after checking its durable version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// The game whose state is represented.
    pub game_id: GameId,
    /// The permanent bindings used to derive each caller's side.
    pub participants: Participants,
    /// The accepted command count. New games start at zero; the limit is `i64::MAX`.
    pub seq: u64,
    /// The retained board and hands, including after game completion.
    pub position: PositionView,
    /// The side to move in the retained position, including after completion.
    pub turn: Player,
    /// Whether the side to move in the retained position is checked.
    pub in_check: bool,
    /// The terminal result, or `None` while the game continues.
    pub result: Option<GameResult>,
    /// The legal moves, including optional nonpromotions. Empty after completion.
    pub legal_moves: Vec<Move>,
}

/// A command, lookup, restoration, or durability failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationError {
    /// A game with this ID already exists.
    AlreadyExists,
    /// The supplied user is not bound to either side.
    NonParticipant,
    /// The two participant IDs are equal.
    InvalidParticipants,
    /// Durable data or schema cannot be decoded or validated by replay.
    Corrupt,
    /// The game does not exist.
    NotFound,
    /// Storage or an actor is unavailable, or the commit outcome is uncertain.
    /// Retry mutations with identical contents and the same request ID.
    Unavailable,
    /// An internal implementation invariant was violated.
    Internal,
    /// The command's required version differs from the durable version.
    SequenceMismatch {
        /// The sequence supplied by the caller.
        expected: u64,
        /// The durable sequence observed by the service or store.
        actual: u64,
    },
    /// This request ID was accepted with different command contents.
    RequestConflict,
    /// The domain rejected the command without changing the candidate game.
    Domain(CommandError),
    /// A new command would exceed the portable sequence limit of `i64::MAX`.
    SequenceExhausted,
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyExists => f.write_str("resource already exists"),
            Self::NonParticipant => f.write_str("user is not a game participant"),
            Self::InvalidParticipants => f.write_str("participants must be distinct user IDs"),
            Self::Corrupt => f.write_str("durable game history is corrupt"),
            Self::NotFound => f.write_str("game not found"),
            Self::Unavailable => {
                f.write_str("storage or actor unavailable; retry the same request ID because the command may have committed")
            }
            Self::Internal => f.write_str("internal application invariant violated"),
            Self::SequenceMismatch { expected, actual } => {
                write!(f, "sequence mismatch: expected {expected}, actual {actual}")
            }
            Self::RequestConflict => {
                f.write_str("request ID was accepted with different command contents")
            }
            Self::Domain(error) => error.fmt(f),
            Self::SequenceExhausted => f.write_str("game sequence exhausted"),
        }
    }
}
impl Error for ApplicationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Domain(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn participant_bindings_are_distinct_and_preserve_sides() {
        let black = UserId::new("black").unwrap();
        let white = UserId::new("white").unwrap();
        assert_eq!(
            Participants::new(black.clone(), black.clone()),
            Err(ApplicationError::InvalidParticipants)
        );
        let participants = Participants::new(black.clone(), white.clone()).unwrap();
        assert_eq!(participants.black(), &black);
        assert_eq!(participants.white(), &white);
        assert_eq!(participants.side(&black), Ok(Player::Black));
        assert_eq!(participants.side(&white), Ok(Player::White));
        assert_eq!(
            participants.side(&UserId::new("outsider").unwrap()),
            Err(ApplicationError::NonParticipant)
        );
    }
}
