#![cfg(test)]

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
    async fn join_matchmaking(
        &self,
        request: tonic::Request<proto::JoinMatchmakingRequest>,
    ) -> Result<tonic::Response<proto::JoinMatchmakingResponse>, tonic::Status> {
        self.inner.join_matchmaking(request).await
    }
    async fn get_matchmaking(
        &self,
        request: tonic::Request<proto::MatchmakingOwner>,
    ) -> Result<tonic::Response<proto::MatchmakingCurrent>, tonic::Status> {
        self.inner.get_matchmaking(request).await
    }
    async fn get_matchmaking_ticket(
        &self,
        request: tonic::Request<proto::MatchmakingRequest>,
    ) -> Result<tonic::Response<proto::MatchmakingTicket>, tonic::Status> {
        self.inner.get_matchmaking_ticket(request).await
    }
    async fn heartbeat_matchmaking(
        &self,
        request: tonic::Request<proto::HeartbeatMatchmakingRequest>,
    ) -> Result<tonic::Response<proto::MatchmakingTicket>, tonic::Status> {
        self.inner.heartbeat_matchmaking(request).await
    }
    async fn cancel_matchmaking(
        &self,
        request: tonic::Request<proto::MatchmakingRequest>,
    ) -> Result<tonic::Response<proto::MatchmakingTicket>, tonic::Status> {
        self.inner.cancel_matchmaking(request).await
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
            inner: GameRpc::new(GameService::new(games.clone()), games.clone()),
            hold: Mutex::new(hold),
        });
        let game_task = tokio::spawn(async move {
            Server::builder()
                .add_service(service)
                .serve_with_incoming(TcpListenerStream::new(game_listener))
                .await
                .unwrap();
        });
        let client = GameServiceClient::connect(game_url.clone()).await.unwrap();
        let game = uuid::Uuid::new_v4().to_string();
        pg_test_support::seed_game(
            game_database.url(),
            &GameId::new(game.clone()).unwrap(),
            &game_application::Participants::new(
                UserId::new(black.id.clone()).unwrap(),
                UserId::new(white.id.clone()).unwrap(),
            )
            .unwrap(),
        )
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

#[path = "bootstrap-contract/tests.rs"]
mod realtime_game_feed_bootstrap_contract;

#[path = "subscription-pump/tests.rs"]
mod realtime_game_feed_subscription_pump;

#[path = "../game-snapshots-access/access-policy/tests.rs"]
mod game_snapshots_access_access_policy;

#[path = "../account-session/realtime-authentication/tests.rs"]
mod account_session_realtime_authentication;

#[path = "cursor/tests.rs"]
mod realtime_game_feed_cursor;

#[path = "../realtime-session-commands-limits/shutdown-contract/tests.rs"]
mod realtime_session_commands_limits_shutdown_contract;

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

#[path = "../realtime-session-commands-limits/upgrade-permits/tests.rs"]
mod realtime_session_commands_limits_upgrade_permits;

#[path = "../realtime-session-commands-limits/command-input/tests.rs"]
mod realtime_session_commands_limits_command_input;

#[path = "../realtime-session-commands-limits/outgoing/tests.rs"]
mod realtime_session_commands_limits_outgoing;

#[path = "../matchmaking-tickets/browser-owner/tests.rs"]
mod matchmaking_tickets_browser_owner;

#[path = "../matchmaking-tickets/browser-schema/tests.rs"]
mod matchmaking_tickets_browser_schema;
