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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MakeMove {
    pub request_id: RequestId,
    pub expected_seq: u64,
    pub player: Player,
    pub movement: Move,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resign {
    pub request_id: RequestId,
    pub expected_seq: u64,
    pub player: Player,
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
            Self::AlreadyExists => f.write_str("game already exists"),
            Self::NotFound => f.write_str("game not found"),
            Self::Unavailable => {
                f.write_str("game actor unavailable; an enqueued command may have executed")
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
