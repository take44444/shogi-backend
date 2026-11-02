use std::{error::Error, fmt};

use shogi_domain::{CommandError, GameResult, Move, Player, PositionView};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidId;

impl fmt::Display for InvalidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ID must contain 1..=128 bytes and no whitespace")
    }
}
impl Error for InvalidId {}

macro_rules! id {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, InvalidId> {
                let value = value.into();
                if value.is_empty() || value.len() > 128 || value.chars().any(char::is_whitespace) {
                    return Err(InvalidId);
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

id!(GameId);
id!(RequestId);
id!(UserId);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Participants {
    pub black: UserId,
    pub white: UserId,
}

impl Participants {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MakeMove {
    pub request_id: RequestId,
    pub expected_seq: u64,
    pub user_id: UserId,
    pub movement: Move,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resign {
    pub request_id: RequestId,
    pub expected_seq: u64,
    pub user_id: UserId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Acceptance {
    pub seq: u64,
    pub in_check: bool,
    pub result: Option<GameResult>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub game_id: GameId,
    pub participants: Participants,
    pub seq: u64,
    pub position: PositionView,
    pub turn: Player,
    pub in_check: bool,
    pub result: Option<GameResult>,
    pub legal_moves: Vec<Move>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationError {
    AlreadyExists,
    UserNotFound,
    NonParticipant,
    InvalidParticipants,
    Corrupt,
    NotFound,
    Unavailable,
    SequenceMismatch { expected: u64, actual: u64 },
    RequestConflict,
    Domain(CommandError),
    SequenceExhausted,
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyExists => f.write_str("resource already exists"),
            Self::UserNotFound => f.write_str("user not found"),
            Self::NonParticipant => f.write_str("user is not a game participant"),
            Self::InvalidParticipants => f.write_str("participants must be distinct existing users"),
            Self::Corrupt => f.write_str("durable game history is corrupt"),
            Self::NotFound => f.write_str("game not found"),
            Self::Unavailable => {
                f.write_str("storage or actor unavailable; retry the same request ID because the command may have committed")
            }
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
