use crate::{conversion::application_status, proto};
use game_application::{GameId, GameService, MakeMove, Participants, RequestId, Resign, UserId};
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
}

impl GameRpc {
    /// Wraps the game service.
    /// Clones share its repository and game actor registry.
    /// Construction performs no I/O and does not start a listener.
    pub fn new(games: GameService) -> Self {
        Self { games }
    }
}

#[tonic::async_trait]
impl proto::game_service_server::GameService for GameRpc {
    async fn create_game(
        &self,
        request: Request<proto::CreateGameRequest>,
    ) -> Result<Response<proto::Snapshot>, Status> {
        let request = request.into_inner();
        let id = GameId::new(request.game_id).map_err(invalid)?;
        let participants = Participants {
            black: UserId::new(request.black_user_id).map_err(invalid)?,
            white: UserId::new(request.white_user_id).map_err(invalid)?,
        };
        let snapshot = self
            .games
            .create(id, participants)
            .await
            .map_err(application_status)?;
        Ok(Response::new(snapshot.into()))
    }

    async fn get_snapshot(
        &self,
        request: Request<proto::GetSnapshotRequest>,
    ) -> Result<Response<proto::Snapshot>, Status> {
        let id = GameId::new(request.into_inner().game_id).map_err(invalid)?;
        let snapshot = self.games.snapshot(&id).await.map_err(application_status)?;
        Ok(Response::new(snapshot.into()))
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
