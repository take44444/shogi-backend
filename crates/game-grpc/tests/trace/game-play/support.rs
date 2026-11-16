#![cfg(test)]

use game_application::GameService;
use game_grpc::{
    GameRpc,
    proto::{self, game_service_client::GameServiceClient, game_service_server::GameServiceServer},
};
use game_persistence::SeaOrmStore;
use std::sync::Arc;
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{
    Code,
    transport::{Channel, Server},
};

struct Client {
    inner: GameServiceClient<Channel>,
    url: String,
}
impl std::ops::Deref for Client {
    type Target = GameServiceClient<Channel>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl std::ops::DerefMut for Client {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
struct SeedGame {
    game_id: String,
    black_user_id: String,
    white_user_id: String,
}
impl Client {
    async fn seed_game(
        &mut self,
        request: SeedGame,
    ) -> Result<tonic::Response<proto::Snapshot>, tonic::Status> {
        let id = game_application::GameId::new(request.game_id)
            .map_err(|_| tonic::Status::invalid_argument("id"))?;
        let players = game_application::Participants::new(
            game_application::UserId::new(request.black_user_id)
                .map_err(|_| tonic::Status::invalid_argument("user"))?,
            game_application::UserId::new(request.white_user_id)
                .map_err(|_| tonic::Status::invalid_argument("user"))?,
        )
        .map_err(|_| tonic::Status::invalid_argument("players"))?;
        pg_test_support::seed_game(&self.url, &id, &players)
            .await
            .map_err(|e| {
                if e == game_application::ApplicationError::AlreadyExists {
                    tonic::Status::already_exists("fixture")
                } else {
                    tonic::Status::unavailable("fixture")
                }
            })?;
        self.inner
            .get_snapshot(proto::GetSnapshotRequest {
                game_id: id.as_str().into(),
            })
            .await
    }
}

struct TestServer {
    endpoint: String,
    url: String,
    store: Arc<SeaOrmStore>,
    _database: Option<pg_test_support::TestDatabase>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), tonic::transport::Error>>>,
}

impl TestServer {
    async fn start() -> Self {
        let database = pg_test_support::TestDatabase::new().await;
        let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
        let games = GameService::new(store.clone());
        let mut server = Self::start_service(games, store, database.url()).await;
        server._database = Some(database);
        server
    }

    async fn start_service(games: GameService, store: Arc<SeaOrmStore>, url: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = oneshot::channel();
        let rpc = GameRpc::new(games, store.clone());
        let task = tokio::spawn(
            Server::builder()
                .add_service(GameServiceServer::new(rpc))
                .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                    let _ = stopped.await;
                }),
        );
        Self {
            endpoint,
            store,
            url: url.into(),
            _database: None,
            stop: Some(stop),
            task: Some(task),
        }
    }

    async fn client(&self) -> Client {
        Client {
            inner: GameServiceClient::connect(self.endpoint.clone())
                .await
                .unwrap(),
            url: self.url.clone(),
        }
    }

    async fn shutdown(mut self) {
        self.stop.take().unwrap().send(()).unwrap();
        self.task.take().unwrap().await.unwrap().unwrap();
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

fn command(
    game: &str,
    request: &str,
    seq: u64,
    player: proto::Player,
    usi: &str,
) -> proto::MakeMoveRequest {
    proto::MakeMoveRequest {
        game_id: game.into(),
        request_id: request.into(),
        expected_seq: Some(seq),
        user_id: match player {
            proto::Player::Black => "black",
            proto::Player::White => "white",
            _ => "",
        }
        .into(),
        usi: usi.into(),
    }
}
fn resign(game: &str, request: &str, seq: u64, player: proto::Player) -> proto::ResignRequest {
    proto::ResignRequest {
        game_id: game.into(),
        request_id: request.into(),
        expected_seq: Some(seq),
        user_id: match player {
            proto::Player::Black => "black",
            proto::Player::White => "white",
            _ => "",
        }
        .into(),
    }
}
async fn create(client: &mut Client, game: &str) -> proto::Snapshot {
    client
        .seed_game(SeedGame {
            game_id: game.into(),
            black_user_id: "black".into(),
            white_user_id: "white".into(),
        })
        .await
        .unwrap()
        .into_inner()
}
async fn snapshot(client: &mut Client, game: &str) -> proto::Snapshot {
    client
        .get_snapshot(proto::GetSnapshotRequest {
            game_id: game.into(),
        })
        .await
        .unwrap()
        .into_inner()
}
fn hand(snapshot: &proto::Snapshot, player: proto::Player, piece: proto::HandPiece) -> u32 {
    snapshot
        .hands
        .iter()
        .find(|hand| hand.player == player as i32 && hand.piece == piece as i32)
        .unwrap()
        .count
}
fn piece(snapshot: &proto::Snapshot, file: u32, rank: u32) -> &proto::BoardPiece {
    snapshot
        .board
        .iter()
        .find(|piece| piece.file == file && piece.rank == rank)
        .unwrap()
}

#[path = "domain-play/tests.rs"]
mod game_play_domain_play;

#[path = "notation/tests.rs"]
mod game_play_notation;

#[path = "rpc-errors/tests.rs"]
mod game_play_rpc_errors;

#[path = "../game-command-recovery/replay/tests.rs"]
mod game_command_recovery_replay;

#[path = "../game-command-recovery/actor-apply/tests.rs"]
mod game_command_recovery_actor_apply;

#[path = "binding/tests.rs"]
mod game_play_binding;

#[path = "../matchmaking-tickets/cancel/tests.rs"]
mod matchmaking_tickets_cancel;
