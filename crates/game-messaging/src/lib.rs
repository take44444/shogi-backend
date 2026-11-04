//! Version-one JSON notifications over Valkey Pub/Sub. A PUBLISH response
//! confirms acceptance, not subscriber delivery or durable storage.
#![warn(missing_docs)]
use async_trait::async_trait;
use game_application::{
    CommittedEvent, DomainEvent, EventKind, GameId, NotificationPublisher, PublishError,
};
use serde::Serialize;
use shogi_domain::{FinishReason, GameResult, Player};
use std::time::Duration;

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
mod tests {
    use super::*;
    use game_application::{RequestId, UserId};
    use serde_json::json;
    fn event(game: &str, event: DomainEvent) -> CommittedEvent {
        CommittedEvent {
            game_id: GameId::new(game).unwrap(),
            request_id: RequestId::new("r\"\0").unwrap(),
            user_id: UserId::new("user").unwrap(),
            seq: 4,
            event,
        }
    }
    #[test]
    fn version_one_json_has_explicit_moves_results_and_identity() {
        let result = GameResult {
            winner: Player::White,
            reason: FinishReason::Checkmate,
        };
        let value: serde_json::Value = serde_json::from_slice(
            &encode(&event(
                "game",
                DomainEvent::MoveCommitted {
                    movement: "2b3c+".parse().unwrap(),
                    in_check: true,
                    result: Some(result),
                },
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            value,
            json!({"schema_version":1,"event_id":"67616d65:4:move_committed", "type":"move_committed",
            "game_id":"game","seq":4,"request_id":"r\"\0","user_id":"user","movement":"2b3c+","in_check":true,
            "result":{"winner":"white","reason":"checkmate"}})
        );
        let end: serde_json::Value = serde_json::from_slice(
            &encode(&event(
                "game",
                DomainEvent::GameFinished {
                    result: GameResult {
                        winner: Player::Black,
                        reason: FinishReason::Resignation,
                    },
                },
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(end["event_id"], "67616d65:4:game_finished");
        assert_eq!(
            end["result"],
            json!({"winner":"black","reason":"resignation"})
        );
        assert!(end.get("movement").is_none());
        assert!(end.get("in_check").is_none());
        let drop: serde_json::Value = serde_json::from_slice(
            &encode(&event(
                "game",
                DomainEvent::MoveCommitted {
                    movement: "B*4e".parse().unwrap(),
                    in_check: false,
                    result: None,
                },
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(drop["movement"], "B*4e");
        assert!(drop["result"].is_null());
    }
    #[test]
    fn channels_encode_every_id_byte_without_collisions_or_glob_metacharacters() {
        let inputs = ["a.b", "a*b", "a>b", "棋\0\"", "6162", "ab"];
        let subjects: std::collections::HashSet<_> = inputs
            .into_iter()
            .map(|id| {
                let event = event(
                    id,
                    DomainEvent::GameFinished {
                        result: GameResult {
                            winner: Player::Black,
                            reason: FinishReason::Resignation,
                        },
                    },
                );
                let encoded = encode(&event).unwrap();
                let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
                assert_eq!(value["game_id"], id);
                let subject = game_channel(&event.game_id);
                assert_eq!(subject.split('.').count(), 4);
                assert!(
                    subject
                        .split('.')
                        .nth(2)
                        .unwrap()
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit())
                );
                subject
            })
            .collect();
        assert_eq!(subjects.len(), inputs.len());
        assert_eq!(
            game_channel(&GameId::new("棋\0").unwrap()),
            "shogi.games.e6a38b00.events"
        );
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    use game_application::{
        ApplicationError, OutboxItem, OutboxPublisher, OutboxRepository, RequestId, UserId,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use tokio::{
        io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
        sync::oneshot,
    };

    fn event() -> CommittedEvent {
        CommittedEvent {
            game_id: GameId::new("棋.*\0").unwrap(),
            request_id: RequestId::new("move-0").unwrap(),
            user_id: UserId::new("black").unwrap(),
            seq: 1,
            event: DomainEvent::MoveCommitted {
                movement: "7g7f".parse().unwrap(),
                in_check: false,
                result: None,
            },
        }
    }
    struct Queue(Arc<AtomicBool>);
    #[async_trait]
    impl OutboxRepository for Queue {
        type Cursor = ();
        async fn next_pending(&mut self) -> Result<Option<OutboxItem<()>>, ApplicationError> {
            Ok(Some(OutboxItem {
                cursor: (),
                event: event(),
            }))
        }
        async fn mark_published(&mut self, _: &OutboxItem<()>) -> Result<(), ApplicationError> {
            self.0.store(true, Ordering::SeqCst);
            Ok(())
        }
    }
    async fn command(reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>) -> Vec<Vec<u8>> {
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let count: usize = line.trim().strip_prefix('*').unwrap().parse().unwrap();
        let mut args = Vec::new();
        for _ in 0..count {
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            let length: usize = line.trim().strip_prefix('$').unwrap().parse().unwrap();
            let mut bytes = vec![0; length + 2];
            reader.read_exact(&mut bytes).await.unwrap();
            assert_eq!(&bytes[length..], b"\r\n");
            bytes.truncate(length);
            args.push(bytes);
        }
        args
    }
    async fn server(
        reply: &'static [u8],
    ) -> (
        String,
        oneshot::Receiver<()>,
        oneshot::Sender<()>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("redis://{}", listener.local_addr().unwrap());
        let (seen_tx, seen_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let (read, mut write) = socket.into_split();
            let mut reader = BufReader::new(read);
            loop {
                let args = command(&mut reader).await;
                match args[0].as_slice() {
                    b"CLIENT" => write.write_all(b"+OK\r\n").await.unwrap(),
                    b"PUBLISH" => {
                        assert_eq!(args.len(), 3);
                        assert_eq!(args[1], game_channel(&event().game_id).as_bytes());
                        assert_eq!(args[2], encode(&event()).unwrap());
                        break;
                    }
                    other => panic!("unexpected Valkey command {other:?}"),
                }
            }
            seen_tx.send(()).unwrap();
            release_rx.await.unwrap();
            if !reply.is_empty() {
                let _ = write.write_all(reply).await;
                let mut extra = Vec::new();
                reader.read_to_end(&mut extra).await.unwrap();
                assert!(
                    extra.is_empty(),
                    "failed publication must not send another command"
                );
            } else {
                drop(write);
                drop(reader);
            }
            assert!(
                tokio::time::timeout(Duration::from_millis(100), listener.accept())
                    .await
                    .is_err(),
                "transport must not reconnect"
            );
        });
        (url, seen_rx, release_tx, task)
    }
    #[tokio::test]
    async fn outbox_marks_only_after_a_valid_publish_response_including_no_subscribers() {
        for (reply, accepted) in [
            (b":0\r\n".as_slice(), true),
            (b":3\r\n".as_slice(), true),
            (b"-NOPERM publish denied\r\n".as_slice(), false),
            (b"+OK\r\n".as_slice(), false),
            (b"$1\r\n0\r\n".as_slice(), false),
            (b":-1\r\n".as_slice(), false),
            (b"".as_slice(), false),
        ] {
            let (url, seen, release, server) = server(reply).await;
            let transport = ValkeyEventPublisher::connect(&url).await.unwrap();
            let marked = Arc::new(AtomicBool::new(false));
            let queue = Queue(marked.clone());
            let mut job =
                tokio::spawn(
                    async move { OutboxPublisher::new(queue, transport).publish_next().await },
                );
            seen.await.unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(25), &mut job)
                    .await
                    .is_err()
            );
            assert!(!marked.load(Ordering::SeqCst));
            release.send(()).unwrap();
            assert_eq!(job.await.unwrap().is_ok(), accepted);
            assert_eq!(marked.load(Ordering::SeqCst), accepted);
            server.await.unwrap();
        }
    }
    #[tokio::test]
    async fn cancelled_publication_cannot_reuse_a_connection_or_reconnect() {
        let (url, seen, release, server) = server(b":0\r\n").await;
        let mut publisher = ValkeyEventPublisher::connect(&url).await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), publisher.publish(&event()))
                .await
                .is_err()
        );
        seen.await.unwrap();
        assert!(publisher.publish(&event()).await.is_err());
        release.send(()).unwrap();
        server.await.unwrap();
    }
    #[tokio::test]
    async fn publication_timeout_leaves_the_outbox_pending() {
        let (url, seen, release, server) = server(b":0\r\n").await;
        let transport = ValkeyEventPublisher::connect(&url).await.unwrap();
        let marked = Arc::new(AtomicBool::new(false));
        let queue = Queue(marked.clone());
        let job =
            tokio::spawn(
                async move { OutboxPublisher::new(queue, transport).publish_next().await },
            );
        seen.await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(6), job)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        assert!(!marked.load(Ordering::SeqCst));
        release.send(()).unwrap();
        server.await.unwrap();
    }
}
