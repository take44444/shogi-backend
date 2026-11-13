use futures_util::{SinkExt, StreamExt};
use game_application::{
    CommittedEvent, DomainEvent, GameId, GameService, NotificationPublisher, OutboxPublisher,
    RequestId, UserId,
};
use game_grpc::{
    GameRpc,
    proto::{
        self,
        game_service_client::GameServiceClient,
        game_service_server::{GameService as GameRpcService, GameServiceServer},
    },
};
use game_messaging::{ValkeyEventPublisher, ValkeySubscription, game_channel};
use game_persistence::{PostgresOutbox, SeaOrmStore};
use pg_test_support::TestDatabase;
use realtime_gateway::{Gateway, GatewayConfig, Limits};
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
    time::timeout,
};
use tokio_stream::wrappers::TcpListenerStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{
        self, Message,
        client::IntoClientRequest,
        http::{HeaderValue, Request, StatusCode},
    },
};
use tonic::transport::{Channel, Server};
use user_application::{AccountRepository, DisplayName, NewSession, VerifiedIdentity};
use user_http::{Authorization, BrowserConfig, IdentityProvider};
use user_persistence::PostgresAccounts;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
const WAIT: Duration = Duration::from_secs(5);

struct NoOAuth;
#[async_trait::async_trait]
impl IdentityProvider for NoOAuth {
    fn authorize(&self) -> Authorization {
        panic!("OAuth is outside the gateway path")
    }
    async fn exchange(
        &self,
        _: String,
        _: user_application::LoginAttempt,
    ) -> Result<VerifiedIdentity, user_application::Error> {
        panic!("OAuth is outside the gateway path")
    }
}

struct Browser {
    id: String,
    token: String,
}
impl Browser {
    fn cookie(&self) -> String {
        format!("user_session={}", self.token)
    }
}

struct SnapshotHold {
    captured: oneshot::Sender<u64>,
    release: oneshot::Receiver<()>,
}

struct HeldSnapshotRpc {
    inner: GameRpc,
    hold: Mutex<Option<SnapshotHold>>,
}

#[tonic::async_trait]
impl GameRpcService for HeldSnapshotRpc {
    async fn create_game(
        &self,
        request: tonic::Request<proto::CreateGameRequest>,
    ) -> Result<tonic::Response<proto::Snapshot>, tonic::Status> {
        self.inner.create_game(request).await
    }

    async fn get_snapshot(
        &self,
        request: tonic::Request<proto::GetSnapshotRequest>,
    ) -> Result<tonic::Response<proto::Snapshot>, tonic::Status> {
        self.inner.get_snapshot(request).await
    }

    async fn get_snapshot_for_viewer(
        &self,
        request: tonic::Request<proto::ViewerRequest>,
    ) -> Result<tonic::Response<proto::Snapshot>, tonic::Status> {
        let snapshot = self.inner.get_snapshot_for_viewer(request).await?;
        let hold = self.hold.lock().unwrap().take();
        if let Some(hold) = hold {
            hold.captured.send(snapshot.get_ref().seq).unwrap();
            hold.release.await.unwrap();
        }
        Ok(snapshot)
    }

    async fn check_view_access(
        &self,
        request: tonic::Request<proto::ViewerRequest>,
    ) -> Result<tonic::Response<proto::ViewAccess>, tonic::Status> {
        self.inner.check_view_access(request).await
    }

    async fn make_move(
        &self,
        request: tonic::Request<proto::MakeMoveRequest>,
    ) -> Result<tonic::Response<proto::Acceptance>, tonic::Status> {
        self.inner.make_move(request).await
    }

    async fn resign(
        &self,
        request: tonic::Request<proto::ResignRequest>,
    ) -> Result<tonic::Response<proto::Acceptance>, tonic::Status> {
        self.inner.resign(request).await
    }
}

struct Services {
    tasks: Vec<JoinHandle<()>>,
    gateway: Arc<Gateway>,
    accounts: Arc<PostgresAccounts>,
    games: Arc<SeaOrmStore>,
    client: GameServiceClient<Channel>,
    publisher: OutboxPublisher<PostgresOutbox, ValkeyEventPublisher>,
    origin: String,
    user_url: String,
    valkey_url: String,
    game: String,
    black: Browser,
    white: Browser,
    viewer: Browser,
    user_database: TestDatabase,
    game_database: TestDatabase,
}

impl Services {
    async fn start() -> Self {
        Self::with_limits(Limits::default()).await
    }

    async fn with_limits(limits: Limits) -> Self {
        Self::with_snapshot_hold(limits, None).await
    }

    async fn with_snapshot_hold(limits: Limits, hold: Option<SnapshotHold>) -> Self {
        let valkey_url =
            std::env::var("TEST_VALKEY_URL").expect("set TEST_VALKEY_URL to a real Valkey server");
        let user_database = TestDatabase::new().await;
        let game_database = TestDatabase::new().await;
        let accounts = Arc::new(
            PostgresAccounts::connect(user_database.url())
                .await
                .unwrap(),
        );
        let games = Arc::new(SeaOrmStore::connect(game_database.url()).await.unwrap());
        let black = login(&accounts, "black").await;
        let white = login(&accounts, "white").await;
        let viewer = login(&accounts, "viewer").await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let user_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let user_url = format!("http://{}", user_listener.local_addr().unwrap());
        let user_router = user_http::router(
            accounts.clone(),
            Arc::new(NoOAuth),
            BrowserConfig::new(&origin, true).unwrap(),
        );
        let user_task =
            tokio::spawn(async move { axum::serve(user_listener, user_router).await.unwrap() });
        let game_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let game_url = format!("http://{}", game_listener.local_addr().unwrap());
        let service = GameServiceServer::new(HeldSnapshotRpc {
            inner: GameRpc::new(GameService::new(games.clone())),
            hold: Mutex::new(hold),
        });
        let game_task = tokio::spawn(async move {
            Server::builder()
                .add_service(service)
                .serve_with_incoming(TcpListenerStream::new(game_listener))
                .await
                .unwrap();
        });
        let mut client = GameServiceClient::connect(game_url.clone()).await.unwrap();
        let game = uuid::Uuid::new_v4().to_string();
        client
            .create_game(proto::CreateGameRequest {
                game_id: game.clone(),
                black_user_id: black.id.clone(),
                white_user_id: white.id.clone(),
            })
            .await
            .unwrap();
        let publisher = OutboxPublisher::new(
            PostgresOutbox::connect(game_database.url()).await.unwrap(),
            ValkeyEventPublisher::connect(&valkey_url).await.unwrap(),
        );
        let gateway = Gateway::new(GatewayConfig {
            bind: listener.local_addr().unwrap(),
            public_origin: origin.clone(),
            allow_loopback_http: true,
            user_service_url: user_url.clone(),
            game_service_url: game_url,
            valkey_url: valkey_url.clone(),
            limits,
        })
        .unwrap();
        let router = gateway.router();
        let gateway_task =
            tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self {
            tasks: vec![gateway_task, user_task, game_task],
            gateway,
            accounts,
            games,
            client,
            publisher,
            origin,
            user_url,
            valkey_url,
            game,
            black,
            white,
            viewer,
            user_database,
            game_database,
        }
    }

    fn request(&self, browser: &Browser) -> Request<()> {
        let mut request = format!(
            "{}/ws/games/{}",
            self.origin.replacen("http", "ws", 1),
            self.game
        )
        .into_client_request()
        .unwrap();
        let headers = request.headers_mut();
        headers.insert("origin", self.origin.parse().unwrap());
        headers.insert("cookie", browser.cookie().parse().unwrap());
        headers.insert(
            "sec-websocket-protocol",
            HeaderValue::from_static("shogi.realtime.v1"),
        );
        headers.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        request
    }

    async fn connect(&self, browser: &Browser) -> Socket {
        let (socket, response) = timeout(WAIT, connect_async(self.request(browser)))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            response.headers()["sec-websocket-protocol"],
            "shogi.realtime.v1"
        );
        socket
    }

    async fn publish(&mut self) {
        assert!(
            timeout(WAIT, self.publisher.publish_next())
                .await
                .unwrap()
                .unwrap(),
            "committed outbox notification"
        );
    }

    async fn snapshot(&mut self) -> proto::Snapshot {
        self.client
            .get_snapshot(proto::GetSnapshotRequest {
                game_id: self.game.clone(),
            })
            .await
            .unwrap()
            .into_inner()
    }

    async fn visibility(&self, visibility: &str) {
        let db = Database::connect(self.game_database.url()).await.unwrap();
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE games SET visibility=$1 WHERE id=$2",
            [visibility.into(), self.game.clone().into()],
        ))
        .await
        .unwrap();
        db.close().await.unwrap();
    }

    async fn stop(mut self) {
        self.gateway.shutdown();
        for task in self.tasks.drain(..) {
            task.abort();
            let _ = task.await;
        }
        assert!(
            self.gateway.wait_closed(WAIT).await,
            "all gateway connections close within the drain deadline"
        );
        self.accounts.close().await.unwrap();
        self.games.close().await.unwrap();
    }
}

impl Drop for Services {
    fn drop(&mut self) {
        self.gateway.shutdown();
        for task in &self.tasks {
            task.abort();
        }
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
fn hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

async fn login(accounts: &PostgresAccounts, name: &str) -> Browser {
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let profile = accounts
        .login(
            VerifiedIdentity {
                issuer: "https://accounts.google.com".into(),
                subject: uuid::Uuid::new_v4().to_string(),
                initial_name: DisplayName::new(name.into()).unwrap(),
            },
            NewSession {
                token_hash: hash(&token),
                expires_at: now() + 3600,
            },
            now(),
        )
        .await
        .unwrap();
    Browser {
        id: profile.id.as_str().into(),
        token,
    }
}

async fn receive(socket: &mut Socket) -> Value {
    timeout(WAIT, async {
        loop {
            match socket
                .next()
                .await
                .expect("WebSocket stays open")
                .expect("read WebSocket")
            {
                Message::Text(text) => return serde_json::from_str(&text).unwrap(),
                Message::Ping(_) | Message::Pong(_) => {}
                other => panic!("expected JSON, received {other:?}"),
            }
        }
    })
    .await
    .expect("bounded gateway response")
}

async fn initial(socket: &mut Socket, game: &str, seq: &str) {
    let value = receive(socket).await;
    assert_eq!(value["type"], "snapshot");
    assert_eq!(value["game_id"], game);
    assert_eq!(value["seq"], seq);
    assert_eq!(value["board"].as_array().unwrap().len(), 40);
}

async fn send(socket: &mut Socket, value: Value) {
    timeout(WAIT, socket.send(Message::Text(value.to_string().into())))
        .await
        .unwrap()
        .unwrap();
}

fn movement(request: &str, seq: &str, usi: &str) -> Value {
    json!({"type":"move", "request_id":request, "expected_seq":seq, "usi":usi})
}

async fn closed(socket: &mut Socket) {
    timeout(WAIT, async {
        loop {
            match socket.next().await {
                None | Some(Ok(Message::Close(_))) | Some(Err(_)) => return,
                Some(Ok(Message::Text(text))) => assert_eq!(
                    serde_json::from_str::<Value>(&text).unwrap()["type"],
                    "resync_required",
                    "no game data may escape after access/session failure"
                ),
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                other => panic!("unexpected frame while closing {other:?}"),
            }
        }
    })
    .await
    .expect("connection closes within deadline");
}

async fn rejected(request: Request<()>) -> (StatusCode, Vec<u8>) {
    match timeout(WAIT, connect_async(request)).await.unwrap() {
        Err(tungstenite::Error::Http(response)) => (
            response.status(),
            response.body().clone().unwrap_or_default(),
        ),
        result => panic!("expected HTTP rejection, got {result:?}"),
    }
}

#[tokio::test]
async fn committed_move_during_initial_snapshot_arrives_after_the_frozen_snapshot() {
    let (captured, snapshot_ready) = oneshot::channel();
    let (release_snapshot, release) = oneshot::channel();
    let mut services =
        Services::with_snapshot_hold(Limits::default(), Some(SnapshotHold { captured, release }))
            .await;
    let mut viewer = services.connect(&services.viewer).await;
    assert_eq!(timeout(WAIT, snapshot_ready).await.unwrap().unwrap(), 0);
    let mut broker = redis::Client::open(services.valkey_url.clone())
        .unwrap()
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    let game = GameId::new(services.game.clone()).unwrap();
    let (_, subscribers): (String, i64) = redis::cmd("PUBSUB")
        .arg("NUMSUB")
        .arg(game_channel(&game))
        .query_async(&mut broker)
        .await
        .unwrap();
    assert_eq!(
        subscribers, 1,
        "subscription is active while snapshot is held"
    );
    let accepted = services
        .client
        .make_move(proto::MakeMoveRequest {
            game_id: services.game.clone(),
            request_id: "during-bootstrap".into(),
            expected_seq: Some(0),
            usi: "7g7f".into(),
            user_id: services.black.id.clone(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(accepted.seq, 1);
    services.publish().await;
    assert_eq!(services.snapshot().await.seq, 1);
    release_snapshot.send(()).unwrap();
    initial(&mut viewer, &services.game, "0").await;
    let event = receive(&mut viewer).await;
    assert_eq!(event["type"], "event");
    assert_eq!(event["kind"], "move_committed");
    assert_eq!(event["seq"], "1");
    assert_eq!(event["usi"], "7g7f");
    services.stop().await;
}

#[tokio::test]
async fn bounded_valkey_subscription_overflow_is_terminal() {
    let url =
        std::env::var("TEST_VALKEY_URL").expect("set TEST_VALKEY_URL to a real Valkey server");
    let game = GameId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let mut subscription = ValkeySubscription::subscribe(&url, &game, 1).await.unwrap();
    let mut publisher = ValkeyEventPublisher::connect(&url).await.unwrap();
    for seq in 1..=8 {
        publisher
            .publish(&CommittedEvent {
                game_id: game.clone(),
                request_id: RequestId::new(format!("overflow-{seq}")).unwrap(),
                user_id: UserId::new("black").unwrap(),
                seq,
                event: DomainEvent::MoveCommitted {
                    movement: "7g7f".parse().unwrap(),
                    in_check: false,
                    result: None,
                },
            })
            .await
            .unwrap();
    }
    assert!(timeout(WAIT, subscription.next()).await.unwrap().is_err());
    assert!(timeout(WAIT, subscription.next()).await.unwrap().is_err());
}

#[tokio::test]
async fn participants_and_spectator_receive_committed_moves_and_resignation() {
    let mut services = Services::start().await;
    let mut black = services.connect(&services.black).await;
    let mut white = services.connect(&services.white).await;
    let mut viewer = services.connect(&services.viewer).await;
    for socket in [&mut black, &mut white, &mut viewer] {
        initial(socket, &services.game, "0").await;
    }
    send(&mut black, movement("black-move", "0", "7g7f")).await;
    let accepted = receive(&mut black).await;
    assert_eq!(accepted["type"], "accepted");
    assert_eq!(accepted["request_id"], "black-move");
    assert_eq!(accepted["seq"], "1");
    services.publish().await;
    for socket in [&mut black, &mut white, &mut viewer] {
        let event = receive(socket).await;
        assert_eq!(event["type"], "event");
        assert_eq!(event["kind"], "move_committed");
        assert_eq!(event["seq"], "1");
        assert_eq!(event["usi"], "7g7f");
        assert!(event.get("request_id").is_none());
        assert!(event.get("user_id").is_none());
    }
    send(&mut white, movement("white-move", "1", "3c3d")).await;
    assert_eq!(receive(&mut white).await["seq"], "2");
    services.publish().await;
    for socket in [&mut black, &mut white, &mut viewer] {
        assert_eq!(receive(socket).await["usi"], "3c3d");
    }
    send(
        &mut white,
        json!({"type":"resign", "request_id":"end", "expected_seq":"2"}),
    )
    .await;
    assert_eq!(receive(&mut white).await["type"], "accepted");
    services.publish().await;
    for socket in [&mut black, &mut white, &mut viewer] {
        let event = receive(socket).await;
        assert_eq!(event["seq"], "3");
        assert_eq!(event["kind"], "game_finished");
        assert_eq!(
            event["result"],
            json!({"winner":"black", "reason":"resignation"})
        );
    }
    assert_eq!(services.snapshot().await.seq, 3);
    services.stop().await;
}

#[tokio::test]
async fn lost_acceptance_replays_original_receipt_after_reconnect_without_a_second_move() {
    let mut services = Services::start().await;
    let mut black = services.connect(&services.black).await;
    initial(&mut black, &services.game, "0").await;
    let command = movement("lost-response", "0", "7g7f");
    send(&mut black, command.clone()).await;
    timeout(WAIT, async {
        while services.snapshot().await.seq != 1 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    drop(black);
    let mut retry = services.connect(&services.black).await;
    initial(&mut retry, &services.game, "1").await;
    send(&mut retry, command).await;
    let accepted = receive(&mut retry).await;
    assert_eq!(accepted["type"], "accepted");
    assert_eq!(accepted["request_id"], "lost-response");
    assert_eq!(accepted["seq"], "1");
    assert_eq!(services.snapshot().await.seq, 1);
    services.publish().await;
    assert!(
        !services.publisher.publish_next().await.unwrap(),
        "retry does not enqueue another event"
    );
    services.stop().await;
}

#[tokio::test]
async fn private_games_are_hidden_and_spectators_cannot_mutate_or_spoof_a_participant() {
    let mut services = Services::start().await;
    let mut viewer = services.connect(&services.viewer).await;
    initial(&mut viewer, &services.game, "0").await;
    send(&mut viewer, movement("outsider", "0", "7g7f")).await;
    assert_eq!(receive(&mut viewer).await["code"], "not_participant");
    send(
        &mut viewer,
        json!({"type":"resign", "request_id":"outsider-resign", "expected_seq":"0"}),
    )
    .await;
    assert_eq!(receive(&mut viewer).await["code"], "not_participant");
    let mut forged = movement("forged", "0", "7g7f");
    forged["user_id"] = services.black.id.clone().into();
    send(&mut viewer, forged).await;
    assert_eq!(receive(&mut viewer).await["code"], "invalid_input");
    assert_eq!(services.snapshot().await.seq, 0);
    services.visibility("private").await;
    let denied = proto::ViewerRequest {
        game_id: services.game.clone(),
        viewer_user_id: services.viewer.id.clone(),
    };
    let missing = proto::ViewerRequest {
        game_id: uuid::Uuid::new_v4().to_string(),
        viewer_user_id: services.viewer.id.clone(),
    };
    for request in [denied, missing] {
        assert_eq!(
            services
                .client
                .get_snapshot_for_viewer(request.clone())
                .await
                .unwrap_err()
                .code(),
            tonic::Code::NotFound
        );
        assert_eq!(
            services
                .client
                .check_view_access(request)
                .await
                .unwrap_err()
                .code(),
            tonic::Code::NotFound
        );
    }
    let participant = proto::ViewerRequest {
        game_id: services.game.clone(),
        viewer_user_id: services.black.id.clone(),
    };
    assert_eq!(
        services
            .client
            .get_snapshot_for_viewer(participant.clone())
            .await
            .unwrap()
            .into_inner()
            .seq,
        0
    );
    services
        .client
        .check_view_access(participant)
        .await
        .unwrap();
    let hidden = rejected(services.request(&services.viewer)).await;
    let mut absent = services.request(&services.viewer);
    *absent.uri_mut() = format!(
        "{}/ws/games/{}",
        services.origin.replacen("http", "ws", 1),
        uuid::Uuid::new_v4()
    )
    .parse()
    .unwrap();
    assert_eq!(hidden, rejected(absent).await);
    assert_eq!(hidden.0, StatusCode::NOT_FOUND);
    let mut black = services.connect(&services.black).await;
    initial(&mut black, &services.game, "0").await;
    send(&mut black, movement("participant", "0", "7g7f")).await;
    assert_eq!(receive(&mut black).await["type"], "accepted");
    services.publish().await;
    closed(&mut viewer).await;
    services.stop().await;
}

#[tokio::test]
async fn handshake_requires_exact_origin_unique_session_and_supported_subprotocol() {
    let services = Services::start().await;
    let mut requests = Vec::new();
    let mut request = services.request(&services.black);
    request.headers_mut().remove("origin");
    requests.push((request, StatusCode::FORBIDDEN));
    let mut request = services.request(&services.black);
    request
        .headers_mut()
        .insert("origin", HeaderValue::from_static("https://evil.example"));
    requests.push((request, StatusCode::FORBIDDEN));
    let mut request = services.request(&services.black);
    request
        .headers_mut()
        .append("origin", services.origin.parse().unwrap());
    requests.push((request, StatusCode::FORBIDDEN));
    let mut request = services.request(&services.black);
    request
        .headers_mut()
        .insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
    requests.push((request, StatusCode::FORBIDDEN));
    for protocol in [None, Some("shogi.realtime.v0")] {
        let mut request = services.request(&services.black);
        request.headers_mut().remove("sec-websocket-protocol");
        if let Some(protocol) = protocol {
            request
                .headers_mut()
                .insert("sec-websocket-protocol", protocol.parse().unwrap());
        }
        requests.push((request, StatusCode::FORBIDDEN));
    }
    let mut request = services.request(&services.black);
    request.headers_mut().remove("cookie");
    requests.push((request, StatusCode::UNAUTHORIZED));
    for cookie in [
        format!("{}; {}", services.black.cookie(), services.black.cookie()),
        "user_session=invalid".into(),
        format!("user_session={}", "0".repeat(64)),
    ] {
        let mut request = services.request(&services.black);
        request
            .headers_mut()
            .insert("cookie", cookie.parse().unwrap());
        requests.push((request, StatusCode::UNAUTHORIZED));
    }
    let mut request = services.request(&services.black);
    request
        .headers_mut()
        .append("cookie", services.black.cookie().parse().unwrap());
    requests.push((request, StatusCode::UNAUTHORIZED));
    for (request, expected) in requests {
        assert_eq!(rejected(request).await.0, expected);
    }
    services.stop().await;
}

#[tokio::test]
async fn logout_closes_before_command_and_expiry_closes_before_notification() {
    let mut services = Services::start().await;
    let mut black = services.connect(&services.black).await;
    let mut viewer = services.connect(&services.viewer).await;
    initial(&mut black, &services.game, "0").await;
    initial(&mut viewer, &services.game, "0").await;
    let response = reqwest::Client::new()
        .post(format!("{}/auth/logout", services.user_url))
        .header("origin", &services.origin)
        .header("cookie", services.black.cookie())
        .timeout(WAIT)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    send(&mut black, movement("after-logout", "0", "7g7f")).await;
    closed(&mut black).await;
    assert_eq!(services.snapshot().await.seq, 0);
    assert_eq!(
        rejected(services.request(&services.black)).await.0,
        StatusCode::UNAUTHORIZED
    );
    let db = Database::connect(services.user_database.url())
        .await
        .unwrap();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE sessions SET expires_at=$1 WHERE token_hash=$2",
        [now().into(), hash(&services.viewer.token).into()],
    ))
    .await
    .unwrap();
    db.close().await.unwrap();
    services
        .client
        .resign(proto::ResignRequest {
            game_id: services.game.clone(),
            request_id: "internal-resign".into(),
            expected_seq: Some(0),
            user_id: services.white.id.clone(),
        })
        .await
        .unwrap();
    services.publish().await;
    closed(&mut viewer).await;
    assert_eq!(
        rejected(services.request(&services.viewer)).await.0,
        StatusCode::UNAUTHORIZED
    );
    services.stop().await;
}

#[tokio::test]
async fn malformed_or_gapped_broker_notifications_require_a_new_connection() {
    let services = Services::start().await;
    let game = GameId::new(services.game.clone()).unwrap();
    let mut socket = services.connect(&services.viewer).await;
    initial(&mut socket, &services.game, "0").await;
    let mut publisher = ValkeyEventPublisher::connect(&services.valkey_url)
        .await
        .unwrap();
    publisher
        .publish(&CommittedEvent {
            game_id: game.clone(),
            request_id: RequestId::new("missing-sequence").unwrap(),
            user_id: UserId::new(services.black.id.clone()).unwrap(),
            seq: 2,
            event: DomainEvent::MoveCommitted {
                movement: "7g7f".parse().unwrap(),
                in_check: false,
                result: None,
            },
        })
        .await
        .unwrap();
    closed(&mut socket).await;
    let mut socket = services.connect(&services.viewer).await;
    initial(&mut socket, &services.game, "0").await;
    let mut connection = redis::Client::open(services.valkey_url.clone())
        .unwrap()
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    let subscribers: i64 = redis::cmd("PUBLISH")
        .arg(game_channel(&game))
        .arg(r#"{"schema_version":999}"#)
        .query_async(&mut connection)
        .await
        .unwrap();
    assert!(subscribers >= 1);
    closed(&mut socket).await;
    services.stop().await;
}

#[tokio::test]
async fn shutdown_closes_active_connections_and_refuses_new_upgrades() {
    let services = Services::start().await;
    let mut socket = services.connect(&services.viewer).await;
    initial(&mut socket, &services.game, "0").await;
    services.gateway.shutdown();
    closed(&mut socket).await;
    assert_eq!(
        rejected(services.request(&services.viewer)).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    services.stop().await;
}

#[tokio::test]
async fn shutdown_before_any_connection_refuses_the_first_upgrade() {
    let services = Services::start().await;
    services.gateway.shutdown();
    assert_eq!(
        rejected(services.request(&services.viewer)).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    services.stop().await;
}

async fn connect_after_release(services: &Services, browser: &Browser) -> Socket {
    timeout(WAIT, async {
        loop {
            match connect_async(services.request(browser)).await {
                Ok((socket, _)) => return socket,
                Err(tungstenite::Error::Http(response))
                    if response.status() == StatusCode::TOO_MANY_REQUESTS =>
                {
                    tokio::time::sleep(Duration::from_millis(10)).await
                }
                other => panic!("unexpected reconnect result {other:?}"),
            }
        }
    })
    .await
    .expect("connection and user permits are released")
}

#[tokio::test]
async fn connection_and_user_limits_release_capacity_after_disconnect() {
    let services = Services::with_limits(Limits {
        connections: 2,
        user_connections: 1,
        ..Limits::default()
    })
    .await;
    let mut viewer = services.connect(&services.viewer).await;
    initial(&mut viewer, &services.game, "0").await;
    assert_eq!(
        rejected(services.request(&services.viewer)).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    let mut black = services.connect(&services.black).await;
    initial(&mut black, &services.game, "0").await;
    assert_eq!(
        rejected(services.request(&services.white)).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    drop(viewer);
    let mut white = connect_after_release(&services, &services.white).await;
    initial(&mut white, &services.game, "0").await;
    drop(black);
    let mut black = connect_after_release(&services, &services.black).await;
    initial(&mut black, &services.game, "0").await;
    services.stop().await;
}

#[tokio::test]
async fn oversized_frames_and_input_rate_close_without_committing_commands() {
    let mut services = Services::with_limits(Limits {
        input_bytes: 128,
        messages_per_second: 1,
        ..Limits::default()
    })
    .await;
    let mut oversized = services.connect(&services.black).await;
    initial(&mut oversized, &services.game, "0").await;
    send(&mut oversized, json!({"padding":"x".repeat(1024)})).await;
    closed(&mut oversized).await;
    let mut rate = services.connect(&services.white).await;
    initial(&mut rate, &services.game, "0").await;
    for _ in 0..32 {
        if rate.send(Message::Text("{}".into())).await.is_err() {
            break;
        }
    }
    timeout(WAIT, async {
        loop {
            match rate.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                Some(Ok(Message::Text(raw))) => {
                    let message: Value = serde_json::from_str(&raw).unwrap();
                    assert!(
                        message["type"] == "resync_required"
                            || (message["type"] == "error" && message["code"] == "invalid_input")
                    );
                }
                other => panic!("unexpected response to rate limit {other:?}"),
            }
        }
    })
    .await
    .expect("rate limit closes connection");
    assert_eq!(services.snapshot().await.seq, 0);
    services.stop().await;
}

#[tokio::test]
async fn snapshot_size_limit_closes_without_sending_a_partial_position() {
    let services = Services::with_limits(Limits {
        snapshot_bytes: 1,
        ..Limits::default()
    })
    .await;
    let mut socket = services.connect(&services.viewer).await;
    closed(&mut socket).await;
    services.stop().await;
}
