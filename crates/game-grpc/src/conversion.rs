use crate::proto;
use game_application::{Acceptance, ApplicationError, Snapshot};
use shogi_domain::{CommandError, FinishReason, GameResult, HandPiece, PieceKind, Player, Square};
use tonic::Status;

pub(crate) fn application_status(error: ApplicationError) -> Status {
    let message = error.to_string();
    match error {
        ApplicationError::AlreadyExists | ApplicationError::RequestConflict => {
            Status::already_exists(message)
        }
        ApplicationError::NonParticipant => Status::permission_denied(message),
        ApplicationError::InvalidParticipants => Status::invalid_argument(message),
        ApplicationError::Corrupt => Status::data_loss(message),
        ApplicationError::NotFound => Status::not_found(message),
        ApplicationError::Unavailable => Status::unavailable(message),
        ApplicationError::SequenceMismatch { actual, .. } => {
            let mut status = Status::aborted(message);
            status.metadata_mut().insert("actual-seq", actual.into());
            status
        }
        ApplicationError::Internal
        | ApplicationError::Domain(CommandError::RulesFailure)
        | ApplicationError::SequenceExhausted => Status::internal(message),
        ApplicationError::RatingPoolBusy | ApplicationError::Domain(_) => {
            Status::failed_precondition(message)
        }
    }
}

impl From<Player> for proto::Player {
    fn from(value: Player) -> Self {
        match value {
            Player::Black => Self::Black,
            Player::White => Self::White,
        }
    }
}

impl From<GameResult> for proto::GameResult {
    fn from(value: GameResult) -> Self {
        Self {
            winner: proto::Player::from(value.winner) as i32,
            reason: match value.reason {
                FinishReason::Checkmate => proto::FinishReason::Checkmate,
                FinishReason::Resignation => proto::FinishReason::Resignation,
                FinishReason::Timeout => proto::FinishReason::Timeout,
            } as i32,
        }
    }
}

impl From<Acceptance> for proto::Acceptance {
    fn from(value: Acceptance) -> Self {
        Self {
            clock: Some(value.clock.into()),
            seq: value.seq,
            in_check: value.in_check,
            result: value.result.map(Into::into),
        }
    }
}

impl From<PieceKind> for proto::PieceKind {
    fn from(value: PieceKind) -> Self {
        match value {
            PieceKind::Pawn => Self::Pawn,
            PieceKind::Lance => Self::Lance,
            PieceKind::Knight => Self::Knight,
            PieceKind::Silver => Self::Silver,
            PieceKind::Gold => Self::Gold,
            PieceKind::Bishop => Self::Bishop,
            PieceKind::Rook => Self::Rook,
            PieceKind::King => Self::King,
            PieceKind::PromotedPawn => Self::PromotedPawn,
            PieceKind::PromotedLance => Self::PromotedLance,
            PieceKind::PromotedKnight => Self::PromotedKnight,
            PieceKind::PromotedSilver => Self::PromotedSilver,
            PieceKind::Horse => Self::Horse,
            PieceKind::Dragon => Self::Dragon,
        }
    }
}

impl From<HandPiece> for proto::HandPiece {
    fn from(value: HandPiece) -> Self {
        match value {
            HandPiece::Pawn => Self::Pawn,
            HandPiece::Lance => Self::Lance,
            HandPiece::Knight => Self::Knight,
            HandPiece::Silver => Self::Silver,
            HandPiece::Gold => Self::Gold,
            HandPiece::Bishop => Self::Bishop,
            HandPiece::Rook => Self::Rook,
        }
    }
}

impl From<Snapshot> for proto::Snapshot {
    fn from(value: Snapshot) -> Self {
        let mut board = Vec::new();
        for file in 1..=9 {
            for rank in 1..=9 {
                let square = Square::new(file, rank).expect("board coordinates are in 1..=9");
                if let Some(piece) = value.position.piece_at(square) {
                    board.push(proto::BoardPiece {
                        file: u32::from(file),
                        rank: u32::from(rank),
                        owner: proto::Player::from(piece.owner) as i32,
                        kind: proto::PieceKind::from(piece.kind) as i32,
                    });
                }
            }
        }
        let hands = [Player::Black, Player::White]
            .into_iter()
            .flat_map(|player| {
                let position = &value.position;
                HandPiece::ALL
                    .into_iter()
                    .map(move |piece| proto::HandCount {
                        player: proto::Player::from(player) as i32,
                        piece: proto::HandPiece::from(piece) as i32,
                        count: position.hand_count(player, piece),
                    })
            })
            .collect();
        Self {
            time_control: Some(proto::TimeControl {
                initial_main_ms: value.time_control.initial_main_ms.get(),
                byoyomi_ms: value.time_control.byoyomi_ms.get(),
            }),
            clock: Some(value.clock.into()),
            server_now_ms: value.server_now_ms.get(),
            game_id: value.game_id.as_str().to_owned(),
            black_user_id: value.participants.black().as_str().to_owned(),
            white_user_id: value.participants.white().as_str().to_owned(),
            seq: value.seq,
            board,
            hands,
            turn: proto::Player::from(value.turn) as i32,
            in_check: value.in_check,
            result: value.result.map(Into::into),
            legal_moves: value
                .legal_moves
                .into_iter()
                .map(|movement| movement.to_string())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests;

impl From<game_application::ClockState> for proto::ClockState {
    fn from(value: game_application::ClockState) -> Self {
        use game_application::ClockState;
        Self {
            state: Some(match value {
                ClockState::Running {
                    main_remaining_ms: m,
                    turn_started_at,
                    deadline_at,
                } => proto::clock_state::State::Running(proto::RunningClock {
                    black_main_remaining_ms: m[0].get(),
                    white_main_remaining_ms: m[1].get(),
                    turn_started_at_ms: turn_started_at.get(),
                    deadline_at_ms: deadline_at.get(),
                }),
                ClockState::Stopped {
                    main_remaining_ms: m,
                    byoyomi_remaining_ms: b,
                    stopped_at,
                } => proto::clock_state::State::Stopped(proto::StoppedClock {
                    black_main_remaining_ms: m[0].get(),
                    white_main_remaining_ms: m[1].get(),
                    black_byoyomi_remaining_ms: b[0].get(),
                    white_byoyomi_remaining_ms: b[1].get(),
                    stopped_at_ms: stopped_at.get(),
                }),
            }),
        }
    }
}
