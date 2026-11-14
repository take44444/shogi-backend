use game_application::UserId;
use game_grpc::proto::{self, game_service_client::GameServiceClient};
use serde_json::{Value, json};
use std::time::Duration;
use tonic::{Code, Status, transport::Channel};

#[derive(Clone)]
pub(crate) struct Games {
    channel: Channel,
}

impl Games {
    pub(crate) fn new(endpoint: &str) -> Result<Self, &'static str> {
        let channel = Channel::from_shared(endpoint.to_owned())
            .map_err(|_| "GAME_SERVICE_URL must be a gRPC endpoint")?
            .connect_lazy();
        Ok(Self { channel })
    }
    pub(crate) fn client(&self) -> GameServiceClient<Channel> {
        GameServiceClient::new(self.channel.clone())
    }
    pub(crate) async fn snapshot(
        &self,
        game: &str,
        user: &UserId,
    ) -> Result<proto::Snapshot, Status> {
        let request = proto::ViewerRequest {
            game_id: game.into(),
            viewer_user_id: user.as_str().into(),
        };
        tokio::time::timeout(
            Duration::from_secs(3),
            self.client().get_snapshot_for_viewer(request),
        )
        .await
        .map_err(|_| Status::deadline_exceeded("game read timed out"))?
        .map(|response| response.into_inner())
    }
    pub(crate) async fn check_access(&self, game: &str, user: &UserId) -> Result<(), Status> {
        let request = proto::ViewerRequest {
            game_id: game.into(),
            viewer_user_id: user.as_str().into(),
        };
        tokio::time::timeout(
            Duration::from_secs(3),
            self.client().check_view_access(request),
        )
        .await
        .map_err(|_| Status::deadline_exceeded("game access check timed out"))??;
        Ok(())
    }
    pub(crate) async fn command(
        &self,
        game: &str,
        user: &UserId,
        command: &Command,
    ) -> Result<proto::Acceptance, Status> {
        let mut client = self.client();
        let result = match command {
            Command::Move {
                request_id,
                expected_seq,
                usi,
            } => {
                let request = proto::MakeMoveRequest {
                    game_id: game.into(),
                    request_id: request_id.clone(),
                    expected_seq: Some(*expected_seq),
                    usi: usi.clone(),
                    user_id: user.as_str().into(),
                };
                tokio::time::timeout(Duration::from_secs(3), client.make_move(request)).await
            }
            Command::Resign {
                request_id,
                expected_seq,
            } => {
                let request = proto::ResignRequest {
                    game_id: game.into(),
                    request_id: request_id.clone(),
                    expected_seq: Some(*expected_seq),
                    user_id: user.as_str().into(),
                };
                return tokio::time::timeout(Duration::from_secs(3), client.resign(request))
                    .await
                    .map_err(|_| Status::deadline_exceeded("game command timed out"))?
                    .map(|response| response.into_inner());
            }
        };
        result
            .map_err(|_| Status::deadline_exceeded("game command timed out"))?
            .map(|response| response.into_inner())
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Command {
    Move {
        request_id: String,
        expected_seq: u64,
        usi: String,
    },
    Resign {
        request_id: String,
        expected_seq: u64,
    },
}
impl Command {
    pub(crate) fn id(&self) -> &str {
        match self {
            Self::Move { request_id, .. } | Self::Resign { request_id, .. } => request_id,
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum WireCommand {
    Move {
        request_id: String,
        expected_seq: String,
        usi: String,
    },
    Resign {
        request_id: String,
        expected_seq: String,
    },
}

pub(crate) fn parse_command(raw: &str) -> Result<Command, ()> {
    let wire: WireCommand = serde_json::from_str(raw).map_err(|_| ())?;
    let (request_id, seq) = match &wire {
        WireCommand::Move {
            request_id,
            expected_seq,
            ..
        }
        | WireCommand::Resign {
            request_id,
            expected_seq,
        } => (request_id, expected_seq),
    };
    game_application::RequestId::new(request_id.clone()).map_err(|_| ())?;
    if seq.is_empty()
        || (seq.starts_with('0') && seq.len() > 1)
        || !seq.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(());
    }
    let expected_seq: u64 = seq.parse().map_err(|_| ())?;
    if expected_seq > i64::MAX as u64 {
        return Err(());
    }
    match wire {
        WireCommand::Move {
            request_id, usi, ..
        } => {
            let movement: shogi_domain::Move = usi.parse().map_err(|_| ())?;
            if movement.to_string() != usi {
                return Err(());
            }
            Ok(Command::Move {
                request_id,
                expected_seq,
                usi,
            })
        }
        WireCommand::Resign { request_id, .. } => Ok(Command::Resign {
            request_id,
            expected_seq,
        }),
    }
}

pub(crate) enum CommandOutcome {
    Reply(Value),
    Close,
}
pub(crate) fn command_reply(id: &str, result: Result<proto::Acceptance, Status>) -> CommandOutcome {
    match result {
        Ok(acceptance) => {
            let Ok(result) = result_value(acceptance.result.as_ref()) else {
                return CommandOutcome::Close;
            };
            CommandOutcome::Reply(json!({
                "type": "accepted", "request_id": id, "seq": acceptance.seq.to_string(),
                "in_check": acceptance.in_check, "result": result,
            }))
        }
        Err(status) => {
            let (code, actual_seq) = match status.code() {
                Code::InvalidArgument => ("invalid_input", None),
                Code::PermissionDenied => ("not_participant", None),
                Code::AlreadyExists => ("request_conflict", None),
                Code::Aborted => (
                    "stale_seq",
                    status
                        .metadata()
                        .get("actual-seq")
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_owned),
                ),
                Code::FailedPrecondition => ("rejected", None),
                Code::Unavailable | Code::DeadlineExceeded | Code::Cancelled => {
                    ("outcome_unknown", None)
                }
                Code::NotFound | Code::Internal | Code::DataLoss => return CommandOutcome::Close,
                _ => ("outcome_unknown", None),
            };
            let mut message = json!({ "type": "error", "request_id": id, "code": code });
            if let Some(seq) = actual_seq {
                message["actual_seq"] = seq.into();
            }
            CommandOutcome::Reply(message)
        }
    }
}

fn player(value: i32) -> Result<&'static str, ()> {
    match proto::Player::try_from(value).map_err(|_| ())? {
        proto::Player::Black => Ok("black"),
        proto::Player::White => Ok("white"),
        _ => Err(()),
    }
}
fn reason(value: i32) -> Result<&'static str, ()> {
    match proto::FinishReason::try_from(value).map_err(|_| ())? {
        proto::FinishReason::Checkmate => Ok("checkmate"),
        proto::FinishReason::Resignation => Ok("resignation"),
        _ => Err(()),
    }
}
fn result_value(value: Option<&proto::GameResult>) -> Result<Value, ()> {
    match value {
        None => Ok(Value::Null),
        Some(value) => {
            Ok(json!({ "winner": player(value.winner)?, "reason": reason(value.reason)? }))
        }
    }
}

pub(crate) fn snapshot_json(snapshot: &proto::Snapshot) -> Result<Value, ()> {
    if snapshot.seq > i64::MAX as u64 {
        return Err(());
    }
    let board = snapshot
        .board
        .iter()
        .map(|piece| {
            let kind = proto::PieceKind::try_from(piece.kind).map_err(|_| ())?;
            if kind == proto::PieceKind::Unspecified
                || !(1..=9).contains(&piece.file)
                || !(1..=9).contains(&piece.rank)
            {
                return Err(());
            }
            Ok(
                json!({ "file": piece.file, "rank": piece.rank, "owner": player(piece.owner)?,
            "kind": kind.as_str_name().trim_start_matches("PIECE_KIND_").to_ascii_lowercase() }),
            )
        })
        .collect::<Result<Vec<_>, ()>>()?;
    let hands = snapshot.hands.iter().map(|hand| {
        let piece = proto::HandPiece::try_from(hand.piece).map_err(|_| ())?;
        if piece == proto::HandPiece::Unspecified { return Err(()); }
        Ok(json!({ "player": player(hand.player)?, "piece": piece.as_str_name().trim_start_matches("HAND_PIECE_").to_ascii_lowercase(), "count": hand.count }))
    }).collect::<Result<Vec<_>, ()>>()?;
    let result = match &snapshot.result {
        None => Value::Null,
        Some(value) => json!({ "winner": player(value.winner)?, "reason": reason(value.reason)? }),
    };
    Ok(json!({
        "type": "snapshot", "game_id": snapshot.game_id, "seq": snapshot.seq.to_string(),
        "board": board, "hands": hands, "turn": player(snapshot.turn)?, "in_check": snapshot.in_check,
        "result": result, "legal_moves": snapshot.legal_moves,
        "black_user_id": snapshot.black_user_id, "white_user_id": snapshot.white_user_id,
    }))
}
