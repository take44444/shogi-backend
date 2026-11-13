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
        let participants = Participants::new(
            UserId::new(request.black_user_id).map_err(invalid)?,
            UserId::new(request.white_user_id).map_err(invalid)?,
        )
        .map_err(application_status)?;
        let snapshot = self
            .games
            .create(id, participants)
            .await
            .map_err(application_status)?;
        Ok(Response::new(snapshot.into()))
    }

    async fn create_rated_game(
        &self,
        request: Request<proto::CreateRatedGameRequest>,
    ) -> Result<Response<proto::Snapshot>, Status> {
        let request = request.into_inner();
        let pool = match proto::RatingPool::try_from(request.pool) {
            Ok(proto::RatingPool::TenMinute30) => game_application::rating::Pool::TenMinute30,
            Ok(proto::RatingPool::Byoyomi30) => game_application::rating::Pool::Byoyomi30,
            _ => {
                return Err(Status::invalid_argument(
                    "a supported rating pool is required",
                ));
            }
        };
        let id = GameId::new(request.game_id).map_err(invalid)?;
        let participants = Participants::new(
            UserId::new(request.black_user_id).map_err(invalid)?,
            UserId::new(request.white_user_id).map_err(invalid)?,
        )
        .map_err(application_status)?;
        let snapshot = self
            .games
            .create_rated(id, participants, pool)
            .await
            .map_err(application_status)?;
        Ok(Response::new(snapshot.into()))
    }

    async fn get_rated_game(
        &self,
        request: Request<proto::GetSnapshotRequest>,
    ) -> Result<Response<proto::RatedGameIdentity>, Status> {
        let id = GameId::new(request.into_inner().game_id).map_err(invalid)?;
        let game = self
            .games
            .rated_game(&id)
            .await
            .map_err(application_status)?
            .ok_or_else(|| Status::not_found("rated game not found"))?;
        let start = |state: game_application::rating::RatingState| proto::RatingStart {
            rating: state.rating(),
            completed_rated_games: state.completed(),
        };
        Ok(Response::new(proto::RatedGameIdentity {
            game_id: game.game_id.as_str().into(),
            black_user_id: game.participants.black().as_str().into(),
            white_user_id: game.participants.white().as_str().into(),
            pool: match game.pool {
                game_application::rating::Pool::TenMinute30 => {
                    proto::RatingPool::TenMinute30 as i32
                }
                game_application::rating::Pool::Byoyomi30 => proto::RatingPool::Byoyomi30 as i32,
            },
            black_start: Some(start(game.black_start)),
            white_start: Some(start(game.white_start)),
        }))
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
