//! Version-one JSON notifications over Valkey Pub/Sub. A PUBLISH response
//! confirms acceptance, not subscriber delivery or durable storage.
#![warn(missing_docs)]
use async_trait::async_trait;
use game_application::{
    CommittedEvent, DomainEvent, EventKind, GameId, NotificationPublisher, PublishError, RequestId,
    UserId,
};
use redis::{PushInfo, PushKind, Value};
use serde::{Deserialize, Serialize};
use shogi_domain::{FinishReason, GameResult, Player};
use std::time::Duration;
use tokio::sync::{mpsc, watch};

/// A decoded, validated version-one notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification(pub CommittedEvent);

/// A malformed or unsupported notification requires clients to resynchronize.
#[derive(Debug, thiserror::Error)]
#[error("invalid game notification")]
pub struct DecodeError;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Incoming {
    schema_version: u8,
    event_id: String,
    #[serde(rename = "type")]
    kind: String,
    game_id: String,
    seq: u64,
    request_id: String,
    user_id: String,
    movement: Option<String>,
    in_check: Option<bool>,
    result: Option<IncomingResult>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IncomingResult {
    winner: String,
    reason: String,
}

/// Decodes a bounded publisher message and validates its content and stable identity.
pub fn decode_notification(
    bytes: &[u8],
    expected_game: &GameId,
) -> Result<Notification, DecodeError> {
    if bytes.len() > 4096 {
        return Err(DecodeError);
    }
    let incoming: Incoming = serde_json::from_slice(bytes).map_err(|_| DecodeError)?;
    if incoming.schema_version != 1 || incoming.seq == 0 || incoming.seq > i64::MAX as u64 {
        return Err(DecodeError);
    }
    let game_id = GameId::new(incoming.game_id).map_err(|_| DecodeError)?;
    if &game_id != expected_game
        || incoming.event_id
            != format!(
                "{}:{}:{}",
                hex(game_id.as_str()),
                incoming.seq,
                incoming.kind
            )
    {
        return Err(DecodeError);
    }
    let result = incoming
        .result
        .map(|value| {
            Ok(GameResult {
                winner: match value.winner.as_str() {
                    "black" => Player::Black,
                    "white" => Player::White,
                    _ => return Err(DecodeError),
                },
                reason: match value.reason.as_str() {
                    "checkmate" => FinishReason::Checkmate,
                    "resignation" => FinishReason::Resignation,
                    _ => return Err(DecodeError),
                },
            })
        })
        .transpose()?;
    let event = match incoming.kind.as_str() {
        "move_committed" => {
            let movement = incoming
                .movement
                .ok_or(DecodeError)?
                .parse()
                .map_err(|_| DecodeError)?;
            let in_check = incoming.in_check.ok_or(DecodeError)?;
            if result.is_some_and(|r| r.reason != FinishReason::Checkmate || !in_check) {
                return Err(DecodeError);
            }
            DomainEvent::MoveCommitted {
                movement,
                in_check,
                result,
            }
        }
        "game_finished" => {
            if incoming.movement.is_some() || incoming.in_check.is_some() {
                return Err(DecodeError);
            }
            DomainEvent::GameFinished {
                result: result.ok_or(DecodeError)?,
            }
        }
        _ => return Err(DecodeError),
    };
    Ok(Notification(CommittedEvent {
        game_id,
        request_id: RequestId::new(incoming.request_id).map_err(|_| DecodeError)?,
        user_id: UserId::new(incoming.user_id).map_err(|_| DecodeError)?,
        seq: incoming.seq,
        event,
    }))
}

/// One exact-channel subscription with no automatic reconnect.
pub struct ValkeySubscription {
    _connection: redis::aio::MultiplexedConnection,
    pushed: mpsc::Receiver<PushInfo>,
    overflow: watch::Receiver<bool>,
    channel: String,
    game: GameId,
}
impl ValkeySubscription {
    /// Waits for the server's SUBSCRIBE acknowledgement before returning.
    pub async fn subscribe(
        url: &str,
        game: &GameId,
        capacity: usize,
    ) -> Result<Self, PublishError> {
        if capacity == 0 {
            return Err(PublishError(
                "subscription capacity must be positive".into(),
            ));
        }
        let mut endpoint = url::Url::parse(url).map_err(|e| PublishError(e.to_string()))?;
        if !matches!(endpoint.scheme(), "redis" | "rediss")
            || endpoint
                .query_pairs()
                .any(|(key, value)| key == "protocol" && value != "3")
        {
            return Err(PublishError("Valkey subscription requires RESP3".into()));
        }
        if !endpoint.query_pairs().any(|(key, _)| key == "protocol") {
            endpoint.query_pairs_mut().append_pair("protocol", "3");
        }
        let client =
            redis::Client::open(endpoint.as_str()).map_err(|e| PublishError(e.to_string()))?;
        let (sender, pushed) = mpsc::channel(capacity);
        let (overflow_sender, overflow) = watch::channel(false);
        let config = redis::AsyncConnectionConfig::new()
            .set_connection_timeout(Some(Duration::from_secs(5)))
            .set_response_timeout(Some(Duration::from_secs(5)))
            .set_push_sender(move |info: PushInfo| {
                if sender.try_send(info).is_err() {
                    overflow_sender.send_replace(true);
                    return Err(redis::aio::SendError);
                }
                Ok(())
            });
        let mut connection = tokio::time::timeout(
            Duration::from_secs(5),
            client.get_multiplexed_async_connection_with_config(&config),
        )
        .await
        .map_err(|_| PublishError("Valkey connection timeout".into()))?
        .map_err(|e| PublishError(e.to_string()))?;
        let channel = game_channel(game);
        tokio::time::timeout(Duration::from_secs(5), connection.subscribe(&channel))
            .await
            .map_err(|_| PublishError("Valkey subscription timeout".into()))?
            .map_err(|e| PublishError(e.to_string()))?;
        if *overflow.borrow() {
            return Err(PublishError("Valkey subscription overflow".into()));
        }
        Ok(Self {
            _connection: connection,
            pushed,
            overflow,
            channel,
            game: game.clone(),
        })
    }

    /// Reads the next notification. A transport or decode failure is terminal.
    pub async fn next(&mut self) -> Result<Notification, PublishError> {
        loop {
            if *self.overflow.borrow() {
                return Err(PublishError("Valkey subscription overflow".into()));
            }
            let info = tokio::select! {
                biased;
                _ = self.overflow.changed() => return Err(PublishError("Valkey subscription overflow".into())),
                info = self.pushed.recv() => info.ok_or_else(|| PublishError("Valkey subscription closed".into()))?,
            };
            match info.kind {
                PushKind::Message => {
                    let [Value::BulkString(channel), Value::BulkString(payload)] =
                        info.data.as_slice()
                    else {
                        return Err(PublishError("Invalid Valkey push message".into()));
                    };
                    if channel != self.channel.as_bytes() {
                        return Err(PublishError("Wrong game channel".into()));
                    }
                    return decode_notification(payload, &self.game)
                        .map_err(|_| PublishError("Invalid game notification".into()));
                }
                PushKind::Subscribe => {}
                _ => {
                    return Err(PublishError(
                        "Valkey subscription changed or disconnected".into(),
                    ));
                }
            }
        }
    }
}

fn hex(value: &str) -> String {
    use std::fmt::Write;
    let mut encoded = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(&mut encoded, "{byte:02x}").expect("writing into String");
    }
    encoded
}
/// Exact per-game channel. UTF-8 hex preserves IDs without glob metacharacters.
pub fn game_channel(game: &GameId) -> String {
    format!("shogi.games.{}.events", hex(game.as_str()))
}
fn kind(kind: EventKind) -> &'static str {
    match kind {
        EventKind::MoveCommitted => "move_committed",
        EventKind::GameFinished => "game_finished",
    }
}
#[derive(Serialize)]
struct ResultDto {
    winner: &'static str,
    reason: &'static str,
}
impl From<GameResult> for ResultDto {
    fn from(result: GameResult) -> Self {
        Self {
            winner: match result.winner {
                Player::Black => "black",
                Player::White => "white",
            },
            reason: match result.reason {
                FinishReason::Checkmate => "checkmate",
                FinishReason::Resignation => "resignation",
            },
        }
    }
}
#[derive(Serialize)]
struct Envelope<'a> {
    schema_version: u8,
    event_id: String,
    #[serde(rename = "type")]
    kind: &'static str,
    game_id: &'a str,
    seq: u64,
    request_id: &'a str,
    user_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    movement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    in_check: Option<bool>,
    result: Option<ResultDto>,
}
fn encode(event: &CommittedEvent) -> Result<Vec<u8>, PublishError> {
    let (movement, in_check, result) = match event.event {
        DomainEvent::MoveCommitted {
            movement,
            in_check,
            result,
        } => (Some(movement.to_string()), Some(in_check), result),
        DomainEvent::GameFinished { result } => (None, None, Some(result)),
    };
    serde_json::to_vec(&Envelope {
        schema_version: 1,
        event_id: format!(
            "{}:{}:{}",
            hex(event.game_id.as_str()),
            event.seq,
            kind(event.event.kind())
        ),
        kind: kind(event.event.kind()),
        game_id: event.game_id.as_str(),
        seq: event.seq,
        request_id: event.request_id.as_str(),
        user_id: event.user_id.as_str(),
        movement,
        in_check,
        result: result.map(Into::into),
    })
    .map_err(|e| PublishError(e.to_string()))
}

/// Valkey Pub/Sub adapter with one connection and no automatic reconnection.
/// After an uncertain failure, discard this adapter. Stop the Tokio runtime
/// before releasing exclusive outbox ownership to stop the connection driver.
pub struct ValkeyEventPublisher {
    connection: Option<redis::aio::MultiplexedConnection>,
}
impl ValkeyEventPublisher {
    /// Connects within five seconds using a Redis-compatible URL.
    pub async fn connect(url: &str) -> Result<Self, PublishError> {
        let client = redis::Client::open(url).map_err(|e| PublishError(e.to_string()))?;
        let config = redis::AsyncConnectionConfig::new()
            .set_connection_timeout(Some(Duration::from_secs(5)))
            .set_response_timeout(Some(Duration::from_secs(5)));
        let connection = tokio::time::timeout(
            Duration::from_secs(5),
            client.get_multiplexed_async_connection_with_config(&config),
        )
        .await
        .map_err(|_| PublishError("Valkey connection exceeded five seconds".into()))?
        .map_err(|e| PublishError(e.to_string()))?;
        Ok(Self {
            connection: Some(connection),
        })
    }
}
#[async_trait]
impl NotificationPublisher for ValkeyEventPublisher {
    async fn publish(&mut self, event: &CommittedEvent) -> Result<(), PublishError> {
        let payload = encode(event)?;
        let mut connection = self
            .connection
            .take()
            .ok_or_else(|| PublishError("Valkey connection is no longer usable".into()))?;
        let response: redis::Value = tokio::time::timeout(Duration::from_secs(5), async {
            redis::cmd("PUBLISH")
                .arg(game_channel(&event.game_id))
                .arg(payload)
                .query_async(&mut connection)
                .await
        })
        .await
        .map_err(|_| PublishError("Valkey publication exceeded five seconds".into()))?
        .map_err(|e| PublishError(e.to_string()))?;
        if !matches!(response, redis::Value::Int(count) if count >= 0) {
            return Err(PublishError(
                "Valkey returned an invalid subscriber count".into(),
            ));
        }
        self.connection = Some(connection);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
