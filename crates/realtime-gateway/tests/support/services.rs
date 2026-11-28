pub(super) use futures_util::{SinkExt, StreamExt};
pub(super) use game_application::{
    CommittedEvent, DomainEvent, GameId, GameService, NotificationPublisher, OutboxPublisher,
    RequestId, UserId,
};
pub(super) use game_grpc::{
    GameRpc,
    proto::{
        self,
        game_service_client::GameServiceClient,
        game_service_server::{GameService as GameRpcService, GameServiceServer},
    },
};
pub(super) use game_messaging::{ValkeyEventPublisher, ValkeySubscription, game_channel};
pub(super) use game_persistence::{PostgresOutbox, SeaOrmStore};
pub(super) use pg_test_support::TestDatabase;
pub(super) use realtime_gateway::{Gateway, GatewayConfig, Limits};
pub(super) use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
pub(super) use serde_json::{Value, json};
pub(super) use sha2::{Digest, Sha256};
pub(super) use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
pub(super) use tokio::{
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
    time::timeout,
};
pub(super) use tokio_stream::wrappers::TcpListenerStream;
pub(super) use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{
        self, Message,
        client::IntoClientRequest,
        http::{HeaderValue, Request, StatusCode},
    },
};
pub(super) use tonic::transport::{Channel, Server};
pub(super) use user_application::{AccountRepository, DisplayName, NewSession, VerifiedIdentity};
pub(super) use user_http::{Authorization, BrowserConfig, IdentityProvider};
pub(super) use user_persistence::PostgresAccounts;

pub(super) type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub(super) const WAIT: Duration = Duration::from_secs(5);

pub(super) struct NoOAuth;
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

pub(super) struct Browser {
    pub(super) id: String,
    pub(super) token: String,
}
impl Browser {
    pub(super) fn cookie(&self) -> String {
        format!("user_session={}", self.token)
    }
}

pub(super) struct SnapshotHold {
    pub(super) captured: oneshot::Sender<u64>,
    pub(super) release: oneshot::Receiver<()>,
}

pub(super) struct CommandHold {
    pub(super) captured: oneshot::Sender<()>,
    pub(super) release: oneshot::Receiver<()>,
}

pub(super) struct HeldSnapshotRpc {
    pub(super) inner: GameRpc,
    pub(super) hold: Mutex<Option<SnapshotHold>>,
    pub(super) command_hold: Mutex<Option<CommandHold>>,
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
    async fn heartbeat_matchmaking(
        &self,
        request: tonic::Request<proto::HeartbeatMatchmakingRequest>,
    ) -> Result<tonic::Response<proto::MatchmakingTicket>, tonic::Status> {
        self.inner.heartbeat_matchmaking(request).await
    }
    async fn ready_matchmaking(
        &self,
        request: tonic::Request<proto::MatchmakingRequest>,
    ) -> Result<tonic::Response<proto::MatchmakingTicket>, tonic::Status> {
        self.inner.ready_matchmaking(request).await
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
        let hold = self.command_hold.lock().unwrap().take();
        if let Some(hold) = hold {
            let _ = hold.captured.send(());
            let _ = hold.release.await;
        }
        self.inner.make_move(request).await
    }

    async fn resign(
        &self,
        request: tonic::Request<proto::ResignRequest>,
    ) -> Result<tonic::Response<proto::Acceptance>, tonic::Status> {
        self.inner.resign(request).await
    }
}

pub(super) struct Services {
    pub(super) tasks: Vec<JoinHandle<()>>,
    pub(super) gateway: Arc<Gateway>,
    pub(super) accounts: Arc<PostgresAccounts>,
    pub(super) games: Arc<SeaOrmStore>,
    pub(super) client: GameServiceClient<Channel>,
    pub(super) publisher: OutboxPublisher<PostgresOutbox, ValkeyEventPublisher>,
    pub(super) origin: String,
    pub(super) user_url: String,
    pub(super) valkey_url: String,
    pub(super) game: String,
    pub(super) black: Browser,
    pub(super) white: Browser,
    pub(super) viewer: Browser,
    pub(super) user_database: TestDatabase,
    pub(super) game_database: TestDatabase,
}

impl Services {
    pub(super) async fn start() -> Self {
        Self::with_limits(Limits::default()).await
    }

    pub(super) async fn with_limits(limits: Limits) -> Self {
        Self::with_snapshot_hold(limits, None).await
    }

    pub(super) async fn with_snapshot_hold(limits: Limits, hold: Option<SnapshotHold>) -> Self {
        Self::with_holds(limits, hold, None).await
    }

    pub(super) async fn with_holds(
        limits: Limits,
        hold: Option<SnapshotHold>,
        command_hold: Option<CommandHold>,
    ) -> Self {
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
            command_hold: Mutex::new(command_hold),
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

    pub(super) fn request(&self, browser: &Browser) -> Request<()> {
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

    pub(super) async fn connect(&self, browser: &Browser) -> Socket {
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

    pub(super) async fn publish(&mut self) {
        assert!(
            timeout(WAIT, self.publisher.publish_next())
                .await
                .unwrap()
                .unwrap(),
            "committed outbox notification"
        );
    }

    pub(super) async fn snapshot(&mut self) -> proto::Snapshot {
        self.client
            .get_snapshot(proto::GetSnapshotRequest {
                game_id: self.game.clone(),
            })
            .await
            .unwrap()
            .into_inner()
    }

    pub(super) async fn visibility(&self, visibility: &str) {
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

    pub(super) async fn stop(mut self) {
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

pub(super) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
pub(super) fn hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

pub(super) async fn login(accounts: &PostgresAccounts, name: &str) -> Browser {
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

pub(super) async fn receive(socket: &mut Socket) -> Value {
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

pub(super) async fn initial(socket: &mut Socket, game: &str, seq: &str) {
    let value = receive(socket).await;
    assert_eq!(value["type"], "snapshot");
    assert_eq!(value["game_id"], game);
    assert_eq!(value["seq"], seq);
    assert_eq!(value["board"].as_array().unwrap().len(), 40);
}

pub(super) async fn send(socket: &mut Socket, value: Value) {
    timeout(WAIT, socket.send(Message::Text(value.to_string().into())))
        .await
        .unwrap()
        .unwrap();
}

pub(super) fn movement(request: &str, seq: &str, usi: &str) -> Value {
    json!({"type":"move", "request_id":request, "expected_seq":seq, "usi":usi})
}

pub(super) async fn closed(socket: &mut Socket) {
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

pub(super) async fn rejected(request: Request<()>) -> (StatusCode, Vec<u8>) {
    match timeout(WAIT, connect_async(request)).await.unwrap() {
        Err(tungstenite::Error::Http(response)) => (
            response.status(),
            response.body().clone().unwrap_or_default(),
        ),
        result => panic!("expected HTTP rejection, got {result:?}"),
    }
}

pub(super) async fn connect_after_release(services: &Services, browser: &Browser) -> Socket {
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
