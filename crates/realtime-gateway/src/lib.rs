//! Authenticated browser WebSocket gateway for one game per connection.
mod auth;
mod cursor;
mod game;

use auth::{AuthClient, AuthError, Session};
use axum::{
    Router,
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use cursor::{ConnectionState, Decision, event_json};
use futures_util::{SinkExt, StreamExt};
use game::{Command, CommandOutcome, Games, command_reply, parse_command, snapshot_json};
use game_application::{GameId, UserId};
use game_messaging::{Notification, ValkeySubscription};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch};
use tonic::Code;
use url::Url;

const PROTOCOL: &str = "shogi.realtime.v1";

/// Process resource limits.
#[derive(Clone, Debug)]
pub struct Limits {
    /// Maximum concurrent upgraded connections.
    pub connections: usize,
    /// Maximum connections for one authenticated user.
    pub user_connections: usize,
    /// Buffered notifications and outgoing messages per connection.
    pub queue: usize,
    /// Maximum incoming WebSocket message size in bytes.
    pub input_bytes: usize,
    /// Maximum serialized snapshot size in bytes.
    pub snapshot_bytes: usize,
    /// Maximum incoming messages per second, per connection.
    pub messages_per_second: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            connections: 256,
            user_connections: 8,
            queue: 64,
            input_bytes: 4096,
            snapshot_bytes: 256 * 1024,
            messages_per_second: 10,
        }
    }
}

/// Gateway startup configuration. It contains no database credentials.
#[derive(Clone, Debug)]
pub struct GatewayConfig {
    /// Address to listen on.
    pub bind: SocketAddr,
    /// Exact browser origin used by the reverse proxy.
    pub public_origin: String,
    /// Explicitly permit HTTP on a loopback origin for local development.
    pub allow_loopback_http: bool,
    /// Fixed internal User Service origin.
    pub user_service_url: String,
    /// Fixed internal Game Service gRPC endpoint.
    pub game_service_url: String,
    /// Valkey endpoint for exact game subscriptions.
    pub valkey_url: String,
    /// Process resource limits.
    pub limits: Limits,
}
impl GatewayConfig {
    /// Reads startup configuration from environment variables.
    pub fn from_env() -> Result<Self, &'static str> {
        fn required(key: &'static str) -> Result<String, &'static str> {
            std::env::var(key).ok().filter(|v| !v.is_empty()).ok_or(key)
        }
        fn number(key: &'static str, default: usize, max: usize) -> Result<usize, &'static str> {
            match std::env::var(key) {
                Ok(value) => value
                    .parse()
                    .ok()
                    .filter(|n| (1..=max).contains(n))
                    .ok_or(key),
                Err(_) => Ok(default),
            }
        }
        let bind = std::env::var("GATEWAY_BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8082".into())
            .parse()
            .map_err(|_| "GATEWAY_BIND_ADDR")?;
        let allow_loopback_http = match std::env::var("GATEWAY_ALLOW_LOOPBACK_HTTP").as_deref() {
            Ok("true") => true,
            Ok("false") | Err(_) => false,
            _ => return Err("GATEWAY_ALLOW_LOOPBACK_HTTP"),
        };
        let defaults = Limits::default();
        let limits = Limits {
            connections: number("GATEWAY_MAX_CONNECTIONS", defaults.connections, 100_000)?,
            user_connections: number(
                "GATEWAY_MAX_USER_CONNECTIONS",
                defaults.user_connections,
                1000,
            )?,
            queue: number("GATEWAY_QUEUE_CAPACITY", defaults.queue, 4096)?,
            input_bytes: number("GATEWAY_MAX_INPUT_BYTES", defaults.input_bytes, 1024 * 1024)?,
            snapshot_bytes: number(
                "GATEWAY_MAX_SNAPSHOT_BYTES",
                defaults.snapshot_bytes,
                4 * 1024 * 1024,
            )?,
            messages_per_second: number(
                "GATEWAY_MESSAGES_PER_SECOND",
                defaults.messages_per_second,
                1000,
            )?,
        };
        Ok(Self {
            bind,
            public_origin: required("GATEWAY_PUBLIC_ORIGIN")?,
            allow_loopback_http,
            user_service_url: required("USER_SERVICE_URL")?,
            game_service_url: required("GAME_SERVICE_URL")?,
            valkey_url: required("VALKEY_URL")?,
            limits,
        })
    }
}

/// Cloneable process service and connection manager.
pub struct Gateway {
    origin: String,
    auth: AuthClient,
    games: Games,
    valkey: String,
    limits: Limits,
    total: Arc<Semaphore>,
    users: Arc<Mutex<HashMap<UserId, usize>>>,
    shutdown: watch::Sender<bool>,
}
struct ConnectionPermit {
    _total: OwnedSemaphorePermit,
    user: UserId,
    users: Arc<Mutex<HashMap<UserId, usize>>>,
}
impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        if let Ok(mut users) = self.users.lock()
            && let Some(count) = users.get_mut(&self.user)
        {
            *count -= 1;
            if *count == 0 {
                users.remove(&self.user);
            }
        }
    }
}
impl Gateway {
    /// Validates endpoints and prepares a gateway without opening a listener.
    pub fn new(config: GatewayConfig) -> Result<Arc<Self>, &'static str> {
        let limits = &config.limits;
        if limits.connections == 0
            || limits.user_connections == 0
            || limits.queue == 0
            || limits.input_bytes == 0
            || limits.snapshot_bytes == 0
            || limits.messages_per_second == 0
            || limits.connections > 100_000
            || limits.user_connections > 1000
            || limits.queue > 4096
            || limits.input_bytes > 1024 * 1024
            || limits.snapshot_bytes > 4 * 1024 * 1024
            || limits.messages_per_second > 1000
        {
            return Err("invalid gateway resource limits");
        }
        let origin = Url::parse(&config.public_origin).map_err(|_| "GATEWAY_PUBLIC_ORIGIN")?;
        let secure = origin.scheme() == "https";
        let loopback = matches!(origin.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if (!secure && !(config.allow_loopback_http && origin.scheme() == "http" && loopback))
            || origin.host_str().is_none()
            || origin.path() != "/"
            || origin.query().is_some()
            || origin.fragment().is_some()
            || !origin.username().is_empty()
            || origin.password().is_some()
        {
            return Err(
                "GATEWAY_PUBLIC_ORIGIN must be HTTPS or explicitly permitted loopback HTTP",
            );
        }
        if !config.valkey_url.starts_with("redis://") {
            return Err("VALKEY_URL must be a redis:// URL");
        }
        let auth = AuthClient::new(&config.user_service_url, secure)?;
        let games = Games::new(&config.game_service_url)?;
        let (shutdown, _) = watch::channel(false);
        Ok(Arc::new(Self {
            origin: origin.origin().ascii_serialization(),
            auth,
            games,
            valkey: config.valkey_url,
            limits: config.limits.clone(),
            total: Arc::new(Semaphore::new(config.limits.connections)),
            users: Arc::new(Mutex::new(HashMap::new())),
            shutdown,
        }))
    }
    /// Builds the HTTP router. The caller owns the listener.
    pub fn router(self: &Arc<Self>) -> Router {
        Router::new()
            .route("/ws/games/{game_id}", get(upgrade))
            .with_state(self.clone())
    }
    /// Stops new upgrades and tells active connections to close.
    pub fn shutdown(&self) {
        self.shutdown.send_replace(true);
    }
    /// Waits for in-flight handshakes and upgraded connections to release capacity.
    pub async fn wait_closed(&self, deadline: Duration) -> bool {
        tokio::time::timeout(
            deadline,
            self.total.acquire_many(self.limits.connections as u32),
        )
        .await
        .is_ok_and(|result| result.is_ok())
    }
    fn permit(&self, user: &UserId, total: OwnedSemaphorePermit) -> Option<ConnectionPermit> {
        let mut users = self.users.lock().ok()?;
        let count = users.entry(user.clone()).or_default();
        if *count >= self.limits.user_connections {
            return None;
        }
        *count += 1;
        Some(ConnectionPermit {
            _total: total,
            user: user.clone(),
            users: self.users.clone(),
        })
    }
    async fn recheck(&self, game: &GameId, session: &Session) -> bool {
        self.auth.recheck(session).await.is_ok()
            && self
                .games
                .check_access(game.as_str(), &session.user)
                .await
                .is_ok()
    }
}

fn one_header(headers: &HeaderMap, key: axum::http::header::HeaderName) -> Option<&str> {
    let mut values = headers.get_all(key).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    Some(value)
}
fn browser_headers(headers: &HeaderMap, origin: &str) -> bool {
    if one_header(headers, header::ORIGIN) != Some(origin) {
        return false;
    }
    if one_header(headers, header::HOST) != origin.split_once("://").map(|(_, host)| host) {
        return false;
    }
    if headers.contains_key("sec-fetch-site")
        && one_header(headers, "sec-fetch-site".parse().unwrap()) != Some("same-origin")
    {
        return false;
    }
    let Some(protocols) = one_header(headers, header::SEC_WEBSOCKET_PROTOCOL) else {
        return false;
    };
    protocols
        .split(',')
        .map(str::trim)
        .filter(|v| *v == PROTOCOL)
        .count()
        == 1
}
async fn upgrade(
    State(gateway): State<Arc<Gateway>>,
    Path(raw_game): Path<String>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if *gateway.shutdown.borrow() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    if !browser_headers(&headers, &gateway.origin) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok(game) = GameId::new(raw_game) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(total) = gateway.total.clone().try_acquire_owned() else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    let cookie = match gateway.auth.cookie(&headers) {
        Ok(cookie) => cookie,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    let user = match gateway.auth.authenticate(&cookie).await {
        Ok(user) => user,
        Err(AuthError::Unauthorized) => return StatusCode::UNAUTHORIZED.into_response(),
        Err(AuthError::Unavailable) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    match gateway.games.check_access(game.as_str(), &user).await {
        Ok(()) => {}
        Err(status) if status.code() == Code::NotFound => {
            return StatusCode::NOT_FOUND.into_response();
        }
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
    if *gateway.shutdown.borrow() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let Some(permit) = gateway.permit(&user, total) else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    let session = Session { cookie, user };
    ws.protocols([PROTOCOL])
        .max_message_size(gateway.limits.input_bytes)
        .max_frame_size(gateway.limits.input_bytes)
        .on_upgrade(move |socket| run_connection(gateway, socket, game, session, permit))
        .into_response()
}

enum Outbound {
    Data(Value),
    Resync(&'static str),
    Close,
    Pong(Vec<u8>),
}
async fn writer(
    gateway: Arc<Gateway>,
    game: GameId,
    session: Session,
    mut sink: futures_util::stream::SplitSink<WebSocket, Message>,
    mut outgoing: mpsc::Receiver<Outbound>,
) {
    while let Some(item) = outgoing.recv().await {
        if !gateway.recheck(&game, &session).await {
            break;
        }
        let (message, stop) = match item {
            Outbound::Data(value) => (Message::Text(value.to_string().into()), false),
            Outbound::Resync(reason) => (
                Message::Text(
                    json!({"type":"resync_required","reason":reason})
                        .to_string()
                        .into(),
                ),
                true,
            ),
            Outbound::Close => break,
            Outbound::Pong(bytes) => (Message::Pong(bytes.into()), false),
        };
        if !matches!(
            tokio::time::timeout(Duration::from_secs(3), sink.send(message)).await,
            Ok(Ok(()))
        ) {
            break;
        }
        if stop {
            break;
        }
    }
    let _ = tokio::time::timeout(Duration::from_secs(1), sink.close()).await;
}

async fn pump(
    mut subscription: ValkeySubscription,
    notifications: mpsc::Sender<Result<Notification, ()>>,
) {
    loop {
        let item = subscription.next().await.map_err(|_| ());
        let failure = item.is_err();
        if notifications.try_send(item).is_err() || failure {
            break;
        }
    }
}

enum ReaderExit {
    Disconnected,
    Overflow,
}
async fn reader(
    mut stream: futures_util::stream::SplitStream<WebSocket>,
    commands: mpsc::Sender<(Command, OwnedSemaphorePermit)>,
    outgoing: mpsc::Sender<Outbound>,
    one_command: Arc<Semaphore>,
    limits: Limits,
) -> ReaderExit {
    let mut window = Instant::now();
    let mut count = 0;
    while let Some(frame) = stream.next().await {
        let Ok(frame) = frame else {
            return ReaderExit::Disconnected;
        };
        if window.elapsed() >= Duration::from_secs(1) {
            window = Instant::now();
            count = 0;
        }
        count += 1;
        if count > limits.messages_per_second {
            return ReaderExit::Overflow;
        }
        match frame {
            Message::Text(raw) if raw.len() <= limits.input_bytes => {
                let Ok(command) = parse_command(&raw) else {
                    if outgoing
                        .try_send(Outbound::Data(
                            json!({"type":"error","code":"invalid_input"}),
                        ))
                        .is_err()
                    {
                        return ReaderExit::Overflow;
                    }
                    continue;
                };
                let Ok(permit) = one_command.clone().try_acquire_owned() else {
                    if outgoing
                        .try_send(Outbound::Data(
                            json!({"type":"error","request_id":command.id(),"code":"busy"}),
                        ))
                        .is_err()
                    {
                        return ReaderExit::Overflow;
                    }
                    continue;
                };
                if commands.try_send((command, permit)).is_err() {
                    return ReaderExit::Overflow;
                }
            }
            Message::Ping(bytes) => {
                if outgoing.try_send(Outbound::Pong(bytes.to_vec())).is_err() {
                    return ReaderExit::Overflow;
                }
            }
            Message::Close(_) => return ReaderExit::Disconnected,
            Message::Pong(_) => {}
            _ => {
                if outgoing
                    .try_send(Outbound::Data(
                        json!({"type":"error","code":"invalid_input"}),
                    ))
                    .is_err()
                {
                    return ReaderExit::Overflow;
                }
            }
        }
    }
    ReaderExit::Disconnected
}

async fn command_worker(
    gateway: Arc<Gateway>,
    game: GameId,
    session: Session,
    mut commands: mpsc::Receiver<(Command, OwnedSemaphorePermit)>,
    outgoing: mpsc::Sender<Outbound>,
) -> bool {
    while let Some((command, _permit)) = commands.recv().await {
        if !gateway.recheck(&game, &session).await {
            let _ = outgoing.try_send(Outbound::Close);
            return false;
        }
        let reply = command_reply(
            command.id(),
            gateway
                .games
                .command(game.as_str(), &session.user, &command)
                .await,
        );
        let item = match reply {
            CommandOutcome::Reply(value) => Outbound::Data(value),
            CommandOutcome::Close => Outbound::Close,
        };
        if outgoing.try_send(item).is_err() {
            return true;
        }
    }
    false
}

async fn run_connection(
    gateway: Arc<Gateway>,
    socket: WebSocket,
    game: GameId,
    session: Session,
    _permit: ConnectionPermit,
) {
    let mut shutdown = gateway.shutdown.subscribe();
    if *shutdown.borrow() {
        return;
    }
    let (sink, stream) = socket.split();
    let subscription = match tokio::select! {
        _ = shutdown.changed() => return,
        subscription = ValkeySubscription::subscribe(&gateway.valkey, &game, gateway.limits.queue) => subscription,
    } {
        Ok(subscription) => subscription,
        Err(_) => return,
    };
    let (notify_tx, mut notify_rx) = mpsc::channel(gateway.limits.queue);
    let pump = tokio::spawn(pump(subscription, notify_tx));
    let snapshot = tokio::select! {
        _ = shutdown.changed() => { pump.abort(); return; },
        snapshot = gateway.games.snapshot(game.as_str(), &session.user) => snapshot,
    };
    let Ok(snapshot) = snapshot else {
        pump.abort();
        return;
    };
    if snapshot.game_id != game.as_str() {
        pump.abort();
        return;
    }
    let Ok(value) = snapshot_json(&snapshot) else {
        pump.abort();
        return;
    };
    if value.to_string().len() > gateway.limits.snapshot_bytes {
        pump.abort();
        return;
    }
    let (out_tx, out_rx) = mpsc::channel(gateway.limits.queue);
    let mut writer = tokio::spawn(writer(
        gateway.clone(),
        game.clone(),
        session.clone(),
        sink,
        out_rx,
    ));
    if out_tx.try_send(Outbound::Data(value)).is_err() {
        writer.abort();
        pump.abort();
        return;
    }
    let mut state = ConnectionState::Bootstrapping;
    state.bootstrapped(snapshot.seq, snapshot.result.is_some());
    let (cmd_tx, cmd_rx) = mpsc::channel(1);
    let mut reader = tokio::spawn(reader(
        stream,
        cmd_tx,
        out_tx.clone(),
        Arc::new(Semaphore::new(1)),
        gateway.limits.clone(),
    ));
    let mut worker = tokio::spawn(command_worker(
        gateway.clone(),
        game,
        session,
        cmd_rx,
        out_tx.clone(),
    ));
    let mut resync = None;
    let mut reader_done = false;
    let mut writer_done = false;
    let mut worker_done = false;
    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            result = &mut reader => { reader_done = true; if matches!(result, Ok(ReaderExit::Overflow)) { resync = Some("input_overflow"); } break; },
            _ = &mut writer => { writer_done = true; break; },
            result = &mut worker => { worker_done = true; if matches!(result, Ok(true)) { resync = Some("command_overflow"); } break; },
            item = notify_rx.recv() => {
                let Some(Ok(event)) = item else { resync = Some("subscription_lost"); break; };
                match state.accept(&event) {
                    Decision::Ignore => {},
                    Decision::Resync => { resync = Some("event_gap"); break; },
                    Decision::Deliver => {
                        if out_tx.try_send(Outbound::Data(event_json(&event))).is_err() {
                            resync = Some("slow_client"); break;
                        }
                    }
                }
            }
        }
    }
    state = ConnectionState::Closing;
    let _ = state;
    if !reader_done {
        reader.abort();
        let _ = reader.await;
    }
    if !worker_done {
        worker.abort();
        let _ = worker.await;
    }
    pump.abort();
    let _ = pump.await;
    if let Some(reason) = resync
        && !writer_done
        && out_tx.try_send(Outbound::Resync(reason)).is_ok()
    {
        drop(out_tx);
        if tokio::time::timeout(Duration::from_secs(4), &mut writer)
            .await
            .is_err()
        {
            writer.abort();
            let _ = writer.await;
        }
    } else if !writer_done {
        writer.abort();
        let _ = writer.await;
    }
}
