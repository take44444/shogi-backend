use crate::{conversion::application_status, proto};
use game_application::matchmaking::{
    HeartbeatSequence, MatchmakingError, MatchmakingRepository, Participation, Ticket, TicketId,
    TicketState,
};
use game_application::{GameId, GameService, MakeMove, RequestId, Resign, UserId};
use std::sync::Arc;
use tonic::{Request, Response, Status};

/// Cloneable gRPC facade for [`GameService`].
///
/// Implements [`proto::game_service_server::GameService`] for use with
/// [`proto::game_service_server::GameServiceServer`]. IDs and move syntax are
/// validated before dispatch. User IDs are unauthenticated caller claims. See
/// the [crate documentation](crate) for status mapping and retry contracts.
#[derive(Clone)]
pub struct GameRpc {
    games: GameService,
    matchmaking: Arc<dyn MatchmakingRepository>,
}

impl GameRpc {
    /// Wraps the game service.
    /// Clones share its repository and game actor registry.
    /// Construction performs no I/O and does not start a listener.
    pub fn new(games: GameService, matchmaking: Arc<dyn MatchmakingRepository>) -> Self {
        Self { games, matchmaking }
    }
}

#[tonic::async_trait]
impl proto::game_service_server::GameService for GameRpc {
    async fn join_matchmaking(
        &self,
        request: Request<proto::JoinMatchmakingRequest>,
    ) -> Result<Response<proto::JoinMatchmakingResponse>, Status> {
        let request = request.into_inner();
        let (user, ticket) = owner_ticket(request.user_id, request.ticket_id)?;
        let pool = match proto::RatingPool::try_from(request.pool) {
            Ok(proto::RatingPool::TenMinute30) => game_application::rating::Pool::TenMinute30,
            Ok(proto::RatingPool::Byoyomi30) => game_application::rating::Pool::Byoyomi30,
            _ => return Err(Status::invalid_argument("a supported pool is required")),
        };
        let result = self
            .matchmaking
            .join(user, ticket, pool)
            .await
            .map_err(matchmaking_status)?;
        Ok(Response::new(proto::JoinMatchmakingResponse {
            ticket: Some(ticket_message(result.ticket)),
            created: result.created,
        }))
    }
    async fn get_matchmaking(
        &self,
        request: Request<proto::MatchmakingOwner>,
    ) -> Result<Response<proto::MatchmakingCurrent>, Status> {
        let user = UserId::new(request.into_inner().user_id).map_err(invalid)?;
        let ticket = match self
            .matchmaking
            .current(&user)
            .await
            .map_err(matchmaking_status)?
        {
            Participation::Idle => None,
            Participation::Active(ticket) => Some(ticket_message(ticket)),
        };
        Ok(Response::new(proto::MatchmakingCurrent { ticket }))
    }
    async fn get_matchmaking_ticket(
        &self,
        request: Request<proto::MatchmakingRequest>,
    ) -> Result<Response<proto::MatchmakingTicket>, Status> {
        let request = request.into_inner();
        let (user, id) = owner_ticket(request.user_id, request.ticket_id)?;
        Ok(Response::new(ticket_message(
            self.matchmaking
                .ticket(&user, id)
                .await
                .map_err(matchmaking_status)?,
        )))
    }
    async fn heartbeat_matchmaking(
        &self,
        request: Request<proto::HeartbeatMatchmakingRequest>,
    ) -> Result<Response<proto::MatchmakingTicket>, Status> {
        let request = request.into_inner();
        let (user, id) = owner_ticket(request.user_id, request.ticket_id)?;
        let sequence = HeartbeatSequence::new(request.sequence)
            .ok_or_else(|| Status::invalid_argument("positive heartbeat sequence is required"))?;
        Ok(Response::new(ticket_message(
            self.matchmaking
                .heartbeat(&user, id, sequence)
                .await
                .map_err(matchmaking_status)?,
        )))
    }
    async fn cancel_matchmaking(
        &self,
        request: Request<proto::MatchmakingRequest>,
    ) -> Result<Response<proto::MatchmakingTicket>, Status> {
        let request = request.into_inner();
        let (user, id) = owner_ticket(request.user_id, request.ticket_id)?;
        Ok(Response::new(ticket_message(
            self.matchmaking
                .cancel(&user, id)
                .await
                .map_err(matchmaking_status)?,
        )))
    }

    async fn get_snapshot(
        &self,
        request: Request<proto::GetSnapshotRequest>,
    ) -> Result<Response<proto::Snapshot>, Status> {
        let id = GameId::new(request.into_inner().game_id).map_err(invalid)?;
        let snapshot = self.games.snapshot(&id).await.map_err(application_status)?;
        Ok(Response::new(snapshot.into()))
    }

    async fn get_snapshot_for_viewer(
        &self,
        request: Request<proto::ViewerRequest>,
    ) -> Result<Response<proto::Snapshot>, Status> {
        let request = request.into_inner();
        let id = GameId::new(request.game_id).map_err(invalid)?;
        let viewer = UserId::new(request.viewer_user_id).map_err(invalid)?;
        let snapshot = self
            .games
            .snapshot_for_viewer(&id, &viewer)
            .await
            .map_err(application_status)?;
        Ok(Response::new(snapshot.into()))
    }

    async fn check_view_access(
        &self,
        request: Request<proto::ViewerRequest>,
    ) -> Result<Response<proto::ViewAccess>, Status> {
        let request = request.into_inner();
        let id = GameId::new(request.game_id).map_err(invalid)?;
        let viewer = UserId::new(request.viewer_user_id).map_err(invalid)?;
        self.games
            .check_view_access(&id, &viewer)
            .await
            .map_err(application_status)?;
        Ok(Response::new(proto::ViewAccess {}))
    }

    async fn make_move(
        &self,
        request: Request<proto::MakeMoveRequest>,
    ) -> Result<Response<proto::Acceptance>, Status> {
        let request = request.into_inner();
        let id = GameId::new(request.game_id).map_err(invalid)?;
        let command = MakeMove {
            request_id: RequestId::new(request.request_id).map_err(invalid)?,
            expected_seq: request
                .expected_seq
                .ok_or_else(|| Status::invalid_argument("expected_seq is required"))?,
            user_id: UserId::new(request.user_id).map_err(invalid)?,
            movement: request.usi.parse().map_err(invalid)?,
        };
        let acceptance = self
            .games
            .make_move(&id, command)
            .await
            .map_err(application_status)?;
        Ok(Response::new(acceptance.into()))
    }

    async fn resign(
        &self,
        request: Request<proto::ResignRequest>,
    ) -> Result<Response<proto::Acceptance>, Status> {
        let request = request.into_inner();
        let id = GameId::new(request.game_id).map_err(invalid)?;
        let command = Resign {
            request_id: RequestId::new(request.request_id).map_err(invalid)?,
            expected_seq: request
                .expected_seq
                .ok_or_else(|| Status::invalid_argument("expected_seq is required"))?,
            user_id: UserId::new(request.user_id).map_err(invalid)?,
        };
        let acceptance = self
            .games
            .resign(&id, command)
            .await
            .map_err(application_status)?;
        Ok(Response::new(acceptance.into()))
    }
}

fn invalid(error: impl std::fmt::Display) -> Status {
    Status::invalid_argument(error.to_string())
}

fn owner_ticket(user: String, ticket: String) -> Result<(UserId, TicketId), Status> {
    Ok((
        UserId::new(user).map_err(invalid)?,
        TicketId::parse(&ticket).map_err(invalid)?,
    ))
}
fn matchmaking_status(error: MatchmakingError) -> Status {
    match error {
        MatchmakingError::NotFound => Status::not_found("ticket not found"),
        MatchmakingError::PoolConflict => Status::already_exists("ticket pool conflict"),
        MatchmakingError::Unavailable => {
            Status::unavailable("matchmaking storage unavailable; retry the same request identity")
        }
        MatchmakingError::Corrupt => Status::data_loss("corrupt matchmaking state"),
    }
}
fn ticket_message(ticket: Ticket) -> proto::MatchmakingTicket {
    let mut message = proto::MatchmakingTicket {
        ticket_id: ticket.id.to_string(),
        pool: ticket.pool.map(|p| match p {
            game_application::rating::Pool::TenMinute30 => proto::RatingPool::TenMinute30 as i32,
            game_application::rating::Pool::Byoyomi30 => proto::RatingPool::Byoyomi30 as i32,
        }),
        ..Default::default()
    };
    message.state = match ticket.state {
        TicketState::Waiting(waiting) => {
            message.enqueued_at = Some(waiting.enqueued_at);
            message.expires_at = Some(waiting.expires_at);
            message.heartbeat_sequence = Some(waiting.heartbeat_seq);
            "waiting"
        }
        TicketState::Playing { game_id, side } => {
            message.game_id = Some(game_id.as_str().into());
            message.side = Some(match side {
                shogi_domain::Player::Black => 1,
                shogi_domain::Player::White => 2,
            });
            "playing"
        }
        TicketState::Finished { game_id, side } => {
            message.game_id = Some(game_id.as_str().into());
            message.side = Some(match side {
                shogi_domain::Player::Black => 1,
                shogi_domain::Player::White => 2,
            });
            "finished"
        }
        TicketState::Cancelled => "cancelled",
        TicketState::Expired => "expired",
        TicketState::Refused(reason) => {
            message.refusal_reason = Some(reason.as_str().into());
            "refused"
        }
    }
    .into();
    message
}
