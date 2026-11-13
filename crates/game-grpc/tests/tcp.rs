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

type Client = GameServiceClient<Channel>;

struct TestServer {
    endpoint: String,
    _database: Option<pg_test_support::TestDatabase>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), tonic::transport::Error>>>,
}

impl TestServer {
    async fn start() -> Self {
        let database = pg_test_support::TestDatabase::new().await;
        let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
        let games = GameService::new(store);
        let mut server = Self::start_service(games).await;
        server._database = Some(database);
        server
    }

    async fn start_service(games: GameService) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(
            Server::builder()
                .add_service(GameServiceServer::new(GameRpc::new(games)))
                .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                    let _ = stopped.await;
                }),
        );
        Self {
            endpoint,
            _database: None,
            stop: Some(stop),
            task: Some(task),
        }
    }

    async fn client(&self) -> Client {
        Client::connect(self.endpoint.clone()).await.unwrap()
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
        .create_game(proto::CreateGameRequest {
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

#[tokio::test]
async fn generated_client_plays_capture_promotion_recapture_drop_and_resignation() {
    let server = TestServer::start().await;
    let mut client = server.client().await;
    let initial = create(&mut client, "play").await;
    assert_eq!(initial.game_id, "play");
    assert_eq!(initial.seq, 0);
    assert_eq!(initial.turn, proto::Player::Black as i32);
    assert_eq!(initial.board.len(), 40);
    assert_eq!(initial.hands.len(), 14);
    assert!(initial.hands.iter().all(|hand| hand.count == 0));
    assert_eq!(piece(&initial, 5, 9).kind, proto::PieceKind::King as i32);
    assert!(!initial.in_check);
    assert!(initial.result.is_none());
    assert_eq!(initial.legal_moves.len(), 30);
    for (seq, player, usi) in [
        (0, proto::Player::Black, "7g7f"),
        (1, proto::Player::White, "3c3d"),
        (2, proto::Player::Black, "8h2b+"),
        (3, proto::Player::White, "3a2b"),
        (4, proto::Player::Black, "B*4e"),
    ] {
        let before = snapshot(&mut client, "play").await;
        assert!(before.legal_moves.iter().any(|legal| legal == usi));
        let receipt = client
            .make_move(command("play", &format!("m-{seq}"), seq, player, usi))
            .await
            .unwrap()
            .into_inner();
        let after = snapshot(&mut client, "play").await;
        assert_eq!(receipt.seq, seq + 1);
        assert_eq!(receipt.seq, after.seq);
        assert_eq!(receipt.in_check, after.in_check);
        assert_eq!(receipt.result, after.result);
        match seq {
            2 => {
                assert_eq!(piece(&after, 2, 2).kind, proto::PieceKind::Horse as i32);
                assert_eq!(piece(&after, 2, 2).owner, proto::Player::Black as i32);
                assert_eq!(
                    hand(&after, proto::Player::Black, proto::HandPiece::Bishop),
                    1
                );
            }
            3 => {
                assert_eq!(piece(&after, 2, 2).kind, proto::PieceKind::Silver as i32);
                assert_eq!(piece(&after, 2, 2).owner, proto::Player::White as i32);
                assert_eq!(
                    hand(&after, proto::Player::White, proto::HandPiece::Bishop),
                    1
                );
            }
            4 => {
                assert_eq!(piece(&after, 4, 5).kind, proto::PieceKind::Bishop as i32);
                assert_eq!(piece(&after, 4, 5).owner, proto::Player::Black as i32);
                assert_eq!(
                    hand(&after, proto::Player::Black, proto::HandPiece::Bishop),
                    0
                );
            }
            _ => {}
        }
    }
    let before = snapshot(&mut client, "play").await;
    let receipt = client
        .resign(resign("play", "resign", 5, proto::Player::White))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(receipt.seq, 6);
    assert_eq!(
        receipt.result,
        Some(proto::GameResult {
            winner: proto::Player::Black as i32,
            reason: proto::FinishReason::Resignation as i32
        })
    );
    let finished = snapshot(&mut client, "play").await;
    assert_eq!(finished.board, before.board);
    assert_eq!(finished.hands, before.hands);
    assert_eq!(finished.turn, before.turn);
    assert_eq!(finished.result, receipt.result);
    assert_eq!(finished.seq, receipt.seq);
    assert!(finished.legal_moves.is_empty());
    drop(client);
    server.shutdown().await;
}

#[tokio::test]
async fn malformed_wire_values_are_invalid_arguments_without_changes() {
    let server = TestServer::start().await;
    let mut client = server.client().await;
    let initial = create(&mut client, "validation").await;
    for id in ["".to_owned(), "white space".into(), "x".repeat(129)] {
        assert_eq!(
            client
                .create_game(proto::CreateGameRequest {
                    game_id: id.clone(),
                    black_user_id: "black".into(),
                    white_user_id: "white".into()
                })
                .await
                .unwrap_err()
                .code(),
            Code::InvalidArgument
        );
        assert_eq!(
            client
                .get_snapshot(proto::GetSnapshotRequest { game_id: id })
                .await
                .unwrap_err()
                .code(),
            Code::InvalidArgument
        );
    }
    let valid = command("validation", "correctable", 0, proto::Player::Black, "7g7f");
    let mut malformed = Vec::new();
    let mut missing = valid.clone();
    missing.expected_seq = None;
    malformed.push(missing);
    let mut missing_id = valid.clone();
    missing_id.game_id.clear();
    malformed.push(missing_id);
    let mut bad_id = valid.clone();
    bad_id.request_id = "\u{2003}".into();
    malformed.push(bad_id);
    for value in ["", "bad id", "\u{2003}"] {
        let mut bad_player = valid.clone();
        bad_player.user_id = value.into();
        malformed.push(bad_player);
    }
    for usi in ["", "7g7f+garbage", "resign", "P*0a", "7g7f\n"] {
        let mut bad_usi = valid.clone();
        bad_usi.usi = usi.into();
        malformed.push(bad_usi);
    }
    for request in malformed {
        assert_eq!(
            client.make_move(request).await.unwrap_err().code(),
            Code::InvalidArgument
        );
        assert_eq!(snapshot(&mut client, "validation").await, initial);
    }
    let valid_resign = resign("validation", "correctable", 0, proto::Player::White);
    let mut missing = valid_resign.clone();
    missing.expected_seq = None;
    let mut bad_request_id = valid_resign.clone();
    bad_request_id.request_id.clear();
    let mut bad_game_id = valid_resign.clone();
    bad_game_id.game_id.clear();
    let mut unspecified = valid_resign.clone();
    unspecified.user_id.clear();
    let mut unknown = valid_resign;
    unknown.user_id = "bad id".into();
    for request in [missing, bad_request_id, bad_game_id, unspecified, unknown] {
        assert_eq!(
            client.resign(request).await.unwrap_err().code(),
            Code::InvalidArgument
        );
        assert_eq!(snapshot(&mut client, "validation").await, initial);
    }
    assert_eq!(client.make_move(valid).await.unwrap().into_inner().seq, 1);
    drop(client);
    server.shutdown().await;
}

#[tokio::test]
async fn statuses_sequence_metadata_and_rejected_request_correction() {
    let server = TestServer::start().await;
    let mut client = server.client().await;
    assert_eq!(
        client
            .get_snapshot(proto::GetSnapshotRequest {
                game_id: "absent".into()
            })
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
    assert_eq!(
        client
            .make_move(command("absent", "m", 0, proto::Player::Black, "7g7f"))
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
    assert_eq!(
        client
            .resign(resign("absent", "r", 0, proto::Player::White))
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
    let initial = create(&mut client, "errors").await;
    assert_eq!(
        client
            .create_game(proto::CreateGameRequest {
                game_id: "errors".into(),
                black_user_id: "black".into(),
                white_user_id: "white".into()
            })
            .await
            .unwrap_err()
            .code(),
        Code::AlreadyExists
    );
    for request in [
        command("errors", "retry", 0, proto::Player::White, "3c3d"),
        command("errors", "retry", 0, proto::Player::Black, "7g7e"),
    ] {
        assert_eq!(
            client.make_move(request).await.unwrap_err().code(),
            Code::FailedPrecondition
        );
        assert_eq!(snapshot(&mut client, "errors").await, initial);
    }
    let stale = client
        .make_move(command("errors", "retry", 5, proto::Player::Black, "7g7f"))
        .await
        .unwrap_err();
    assert_eq!(stale.code(), Code::Aborted);
    assert_eq!(stale.metadata().get("actual-seq").unwrap(), "0");
    assert!(stale.message().contains("expected 5, actual 0"));
    let accepted = client
        .make_move(command("errors", "retry", 0, proto::Player::Black, "7g7f"))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(accepted.seq, 1);
    let stale = client
        .resign(resign("errors", "end", 0, proto::Player::White))
        .await
        .unwrap_err();
    assert_eq!(stale.code(), Code::Aborted);
    assert_eq!(stale.metadata().get("actual-seq").unwrap(), "1");
    client
        .resign(resign("errors", "end", 1, proto::Player::White))
        .await
        .unwrap();
    let finished = snapshot(&mut client, "errors").await;
    assert_eq!(
        client
            .make_move(command("errors", "after", 2, proto::Player::White, "3c3d"))
            .await
            .unwrap_err()
            .code(),
        Code::FailedPrecondition
    );
    assert_eq!(
        client
            .resign(resign("errors", "after", 2, proto::Player::Black))
            .await
            .unwrap_err()
            .code(),
        Code::FailedPrecondition
    );
    assert_eq!(snapshot(&mut client, "errors").await, finished);
    drop(client);
    server.shutdown().await;
}

#[tokio::test]
async fn exact_replay_returns_original_receipt_after_finish_and_conflicts_are_rejected() {
    let server = TestServer::start().await;
    let mut client = server.client().await;
    create(&mut client, "replay").await;
    let first = command("replay", "first", 0, proto::Player::Black, "7g7f");
    let receipt = client.make_move(first.clone()).await.unwrap().into_inner();
    client
        .make_move(command("replay", "second", 1, proto::Player::White, "3c3d"))
        .await
        .unwrap();
    let resignation = resign("replay", "end", 2, proto::Player::White);
    let finish = client
        .resign(resignation.clone())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(client.make_move(first).await.unwrap().into_inner(), receipt);
    assert_eq!(
        client.resign(resignation).await.unwrap().into_inner(),
        finish
    );
    for changed in [
        command("replay", "first", 1, proto::Player::Black, "7g7f"),
        command("replay", "first", 0, proto::Player::White, "7g7f"),
        command("replay", "first", 0, proto::Player::Black, "2g2f"),
        command("replay", "end", 2, proto::Player::White, "3c3d"),
    ] {
        assert_eq!(
            client.make_move(changed).await.unwrap_err().code(),
            Code::AlreadyExists
        );
    }
    assert_eq!(
        client
            .resign(resign("replay", "first", 0, proto::Player::Black))
            .await
            .unwrap_err()
            .code(),
        Code::AlreadyExists
    );
    assert_eq!(snapshot(&mut client, "replay").await.seq, 3);
    drop(client);
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn independent_tcp_clients_serialize_competing_commands_and_retry_identically() {
    let server = TestServer::start().await;
    let mut first = server.client().await;
    let mut second = server.client().await;
    create(&mut first, "concurrent").await;
    let a = command("concurrent", "a", 0, proto::Player::Black, "7g7f");
    let b = command("concurrent", "b", 0, proto::Player::Black, "2g2f");
    let (left, right) = tokio::join!(first.make_move(a), second.make_move(b));
    let error = match (left, right) {
        (Ok(receipt), Err(error)) | (Err(error), Ok(receipt)) => {
            assert_eq!(receipt.into_inner().seq, 1);
            error
        }
        result => panic!("expected exactly one acceptance, got {result:?}"),
    };
    assert_eq!(error.code(), Code::Aborted);
    assert_eq!(error.metadata().get("actual-seq").unwrap(), "1");
    let retry = command("concurrent", "white", 1, proto::Player::White, "3c3d");
    let (left, right) = tokio::join!(first.make_move(retry.clone()), second.make_move(retry));
    assert_eq!(left.unwrap().into_inner(), right.unwrap().into_inner());
    assert_eq!(snapshot(&mut first, "concurrent").await.seq, 2);
    let other = create(&mut second, "other").await;
    assert_eq!(other.seq, 0);
    assert_eq!(other.board.len(), 40);
    drop(first);
    drop(second);
    server.shutdown().await;
}

#[tokio::test]
async fn id_bindings_and_permission_statuses_are_enforced_at_the_wire() {
    let server = TestServer::start().await;
    let mut client = server.client().await;
    for (white, expected) in [
        ("black", Code::InvalidArgument),
        ("", Code::InvalidArgument),
    ] {
        assert_eq!(
            client
                .create_game(proto::CreateGameRequest {
                    game_id: "binding".into(),
                    black_user_id: "black".into(),
                    white_user_id: white.into(),
                })
                .await
                .unwrap_err()
                .code(),
            expected
        );
    }
    let independent = client
        .create_game(proto::CreateGameRequest {
            game_id: "independent".into(),
            black_user_id: "new-black".into(),
            white_user_id: "new-white".into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(independent.black_user_id, "new-black");
    assert_eq!(independent.white_user_id, "new-white");
    let initial = create(&mut client, "binding").await;
    assert_eq!(initial.black_user_id, "black");
    assert_eq!(initial.white_user_id, "white");
    let first = command("binding", "first", 0, proto::Player::Black, "7g7f");
    client.make_move(first.clone()).await.unwrap();
    let mut outsider = first;
    outsider.user_id = "outsider".into();
    assert_eq!(
        client.make_move(outsider).await.unwrap_err().code(),
        Code::PermissionDenied
    );
    let mut outsider = resign("binding", "first", 0, proto::Player::White);
    outsider.user_id = "outsider".into();
    assert_eq!(
        client.resign(outsider).await.unwrap_err().code(),
        Code::PermissionDenied
    );
    drop(client);
    server.shutdown().await;
}

#[tokio::test]
async fn generated_tcp_client_restores_database_across_two_server_restarts() {
    let database = pg_test_support::TestDatabase::new().await;
    let url = database.url().to_string();
    let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
    let server = TestServer::start_service(GameService::new(store.clone())).await;
    let mut client = server.client().await;
    create(&mut client, "restart").await;
    let first = command("restart", "first", 0, proto::Player::Black, "7g7f");
    let original = client.make_move(first.clone()).await.unwrap().into_inner();
    let ongoing = snapshot(&mut client, "restart").await;
    drop(client);
    server.shutdown().await;
    store.close().await.unwrap();
    let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
    let server = TestServer::start_service(GameService::new(store.clone())).await;
    let mut client = server.client().await;
    assert_eq!(snapshot(&mut client, "restart").await, ongoing);
    assert_eq!(
        client.make_move(first.clone()).await.unwrap().into_inner(),
        original
    );
    client
        .make_move(command("restart", "white", 1, proto::Player::White, "3c3d"))
        .await
        .unwrap();
    let end = resign("restart", "end", 2, proto::Player::Black);
    let terminal = client.resign(end.clone()).await.unwrap().into_inner();
    let finished = snapshot(&mut client, "restart").await;
    drop(client);
    server.shutdown().await;
    store.close().await.unwrap();
    let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
    let server = TestServer::start_service(GameService::new(store)).await;
    let mut client = server.client().await;
    assert_eq!(snapshot(&mut client, "restart").await, finished);
    assert_eq!(
        client.make_move(first).await.unwrap().into_inner(),
        original
    );
    assert_eq!(client.resign(end).await.unwrap().into_inner(), terminal);
    drop(client);
    server.shutdown().await;
}

#[tokio::test]
async fn rated_rpc_requires_a_pool_preserves_casual_creation_and_exposes_durable_identity() {
    let server = TestServer::start().await;
    let mut client = server.client().await;
    let request = |id: &str, pool: i32| proto::CreateRatedGameRequest {
        game_id: id.into(),
        black_user_id: "black".into(),
        white_user_id: "white".into(),
        pool,
    };
    for pool in [0, 99] {
        assert_eq!(
            client
                .create_rated_game(request("invalid", pool))
                .await
                .unwrap_err()
                .code(),
            Code::InvalidArgument
        );
    }
    client
        .create_rated_game(request("rated", proto::RatingPool::TenMinute30 as i32))
        .await
        .unwrap();
    let identity = client
        .get_rated_game(proto::GetSnapshotRequest {
            game_id: "rated".into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(identity.game_id, "rated");
    assert_eq!(identity.black_user_id, "black");
    assert_eq!(identity.white_user_id, "white");
    assert_eq!(identity.pool, proto::RatingPool::TenMinute30 as i32);
    assert_eq!(
        identity.black_start.unwrap(),
        proto::RatingStart {
            rating: None,
            completed_rated_games: 0
        }
    );
    assert_eq!(
        identity.white_start.unwrap(),
        proto::RatingStart {
            rating: None,
            completed_rated_games: 0
        }
    );
    assert_eq!(
        client
            .create_rated_game(request("overlap", proto::RatingPool::TenMinute30 as i32))
            .await
            .unwrap_err()
            .code(),
        Code::FailedPrecondition
    );
    assert_eq!(
        client
            .create_rated_game(request("rated", proto::RatingPool::TenMinute30 as i32))
            .await
            .unwrap_err()
            .code(),
        Code::AlreadyExists
    );
    client
        .create_rated_game(request("other-pool", proto::RatingPool::Byoyomi30 as i32))
        .await
        .unwrap();
    create(&mut client, "casual").await;
    assert_eq!(
        client
            .get_rated_game(proto::GetSnapshotRequest {
                game_id: "casual".into()
            })
            .await
            .unwrap_err()
            .code(),
        Code::NotFound
    );
    server.shutdown().await;
}
