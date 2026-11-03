//! Version-one JSON and Core NATS publication. An exact event echo confirms
//! server acceptance after socket flush. It does not acknowledge delivery to
//! other subscribers or durability.
#![warn(missing_docs)]
use async_trait::async_trait;
use futures_util::StreamExt;
use game_application::{
    CommittedEvent, DomainEvent, EventKind, EventPublisher, GameId, PublishError,
};
use serde::Serialize;
use shogi_domain::{FinishReason, GameResult, Player};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

fn hex(value: &str) -> String {
    use std::fmt::Write;
    let mut encoded = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(&mut encoded, "{byte:02x}").expect("writing into String");
    }
    encoded
}
/// Exact per-game subject. Lowercase UTF-8 hex preserves arbitrary valid IDs,
/// including dots, wildcards, Unicode, and NUL, in one safe NATS token.
pub fn game_subject(game: &GameId) -> String {
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

/// Waits for the server to process prior commands on this connection.
/// A fresh private inbox echo supplies the round trip that `Client::flush` alone
/// does not provide. Requires publish and subscribe access to private inboxes.
/// Useful for subscription readiness. This does not prove that a preceding
/// publication was accepted; the server may have rejected that publication.
/// Returns an error if the echo is absent after five seconds or the transport fails.
pub async fn server_barrier(client: &async_nats::Client) -> Result<(), PublishError> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let inbox = client.new_inbox();
        let mut subscription = client
            .subscribe(inbox.clone())
            .await
            .map_err(|e| PublishError(e.to_string()))?;
        client
            .publish(inbox, "barrier".into())
            .await
            .map_err(|e| PublishError(e.to_string()))?;
        client
            .flush()
            .await
            .map_err(|e| PublishError(e.to_string()))?;
        let message = subscription
            .next()
            .await
            .ok_or_else(|| PublishError("NATS barrier subscription closed".into()))?;
        if message.payload.as_ref() != b"barrier" {
            return Err(PublishError("unexpected NATS barrier payload".into()));
        }
        Ok(())
    })
    .await
    .map_err(|_| PublishError("NATS server barrier exceeded five seconds".into()))?
}

/// Core NATS adapter. The process must stop its Tokio runtime before releasing
/// the exclusive outbox guard, including on errors or uncertain network timeouts.
/// Publication requires publish and subscribe access to the game event subject.
pub struct NatsEventPublisher {
    client: async_nats::Client,
}
impl NatsEventPublisher {
    /// Connects within five seconds. Returns a transport error on failure.
    pub async fn connect(url: &str) -> Result<Self, PublishError> {
        let attempted = AtomicBool::new(false);
        let options =
            async_nats::ConnectOptions::new().reconnect_to_server_callback(move |_, _| {
                let initial = !attempted.swap(true, Ordering::SeqCst);
                async move {
                    if initial {
                        None
                    } else {
                        std::future::pending().await
                    }
                }
            });
        // async-nats has no disable-reconnect option. Park its reconnect selector
        // before a second dial so an echo cannot confirm a PUB lost on an old socket.
        let client = tokio::time::timeout(Duration::from_secs(5), options.connect(url))
            .await
            .map_err(|_| PublishError("NATS connection exceeded five seconds".into()))?
            .map_err(|e| PublishError(e.to_string()))?;
        Ok(Self { client })
    }
}
#[async_trait]
impl EventPublisher for NatsEventPublisher {
    async fn publish(&mut self, event: &CommittedEvent) -> Result<(), PublishError> {
        let payload = encode(event)?;
        let subject = game_subject(&event.game_id);
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut echoes = self
                .client
                .subscribe(subject.clone())
                .await
                .map_err(|e| PublishError(e.to_string()))?;
            self.client
                .publish(subject.clone(), payload.clone().into())
                .await
                .map_err(|e| PublishError(e.to_string()))?;
            self.client
                .flush()
                .await
                .map_err(|e| PublishError(e.to_string()))?;
            while let Some(message) = echoes.next().await {
                if message.subject.as_str() == subject && message.payload.as_ref() == payload {
                    return Ok(());
                }
            }
            Err(PublishError("NATS event echo subscription closed".into()))
        })
        .await
        .map_err(|_| PublishError("NATS event echo exceeded five seconds".into()))?
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
    fn subjects_encode_every_id_byte_without_collisions_or_nats_metacharacters() {
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
                let subject = game_subject(&event.game_id);
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
            game_subject(&GameId::new("棋\0").unwrap()),
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

    struct Queue {
        event: CommittedEvent,
        marked: Arc<AtomicBool>,
    }
    #[async_trait]
    impl OutboxRepository for Queue {
        type Cursor = ();
        async fn next_pending(&mut self) -> Result<Option<OutboxItem<()>>, ApplicationError> {
            Ok(Some(OutboxItem {
                cursor: (),
                event: self.event.clone(),
            }))
        }
        async fn mark_published(&mut self, _: &OutboxItem<()>) -> Result<(), ApplicationError> {
            self.marked.store(true, Ordering::SeqCst);
            Ok(())
        }
    }
    #[derive(Clone, Copy)]
    enum Behavior {
        Echo,
        Reject,
        Disconnect,
    }
    async fn message(
        write: &mut tokio::net::tcp::OwnedWriteHalf,
        subject: &str,
        sid: &str,
        payload: &[u8],
    ) {
        write
            .write_all(format!("MSG {subject} {sid} {}\r\n", payload.len()).as_bytes())
            .await
            .unwrap();
        write.write_all(payload).await.unwrap();
        write.write_all(b"\r\n").await.unwrap();
    }
    async fn fake_server(
        behavior: Behavior,
    ) -> (
        String,
        oneshot::Receiver<()>,
        oneshot::Sender<()>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("nats://{}", listener.local_addr().unwrap());
        let (observed_tx, observed_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (read, mut write) = stream.into_split();
            let mut reader = BufReader::new(read);
            write
                .write_all(b"INFO {\"server_id\":\"fake\",\"max_payload\":1048576,\"proto\":1}\r\n")
                .await
                .unwrap();
            let mut subscription = None;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).await.unwrap() == 0 {
                    break;
                }
                let fields: Vec<_> = line.split_whitespace().collect();
                match fields[0] {
                    "CONNECT" => {}
                    "PING" => {
                        write.write_all(b"PONG\r\n").await.unwrap();
                    }
                    "SUB" => {
                        subscription = Some((fields[1].to_owned(), fields[2].to_owned()));
                    }
                    "PUB" => {
                        let size: usize = fields.last().unwrap().parse().unwrap();
                        let mut payload = vec![0; size + 2];
                        reader.read_exact(&mut payload).await.unwrap();
                        let (subject, sid) = subscription
                            .take()
                            .expect("subscribe before publishing the event");
                        assert!(subject.starts_with("shogi.games."));
                        assert_eq!(fields[1], subject);
                        let value: serde_json::Value =
                            serde_json::from_slice(&payload[..size]).unwrap();
                        assert_eq!(value["type"], "move_committed");
                        if matches!(behavior, Behavior::Reject) {
                            write.write_all(format!("-ERR 'Permissions Violation for Publish to \"{subject}\"'\r\n").as_bytes()).await.unwrap();
                        }
                        message(&mut write, &subject, &sid, b"barrier").await;
                        message(&mut write, "unrelated.subject", &sid, &payload[..size]).await;
                        observed_tx.send(()).unwrap();
                        release_rx.await.unwrap();
                        if matches!(behavior, Behavior::Disconnect) {
                            drop(write);
                            drop(reader);
                            assert!(
                                tokio::time::timeout(Duration::from_secs(6), listener.accept())
                                    .await
                                    .is_err(),
                                "publisher must not reconnect across an uncertain publication"
                            );
                        } else {
                            if matches!(behavior, Behavior::Echo) {
                                message(&mut write, &subject, &sid, &payload[..size]).await;
                            }
                            let mut rest = Vec::new();
                            let _ = reader.read_to_end(&mut rest).await;
                        }
                        break;
                    }
                    other => panic!("unexpected fake NATS command {other}"),
                }
            }
        });
        (address, observed_rx, release_tx, task)
    }
    #[tokio::test]
    async fn only_an_exact_event_echo_can_mark_even_after_permission_rejection() {
        for behavior in [Behavior::Echo, Behavior::Reject, Behavior::Disconnect] {
            let (address, observed, release, server) = fake_server(behavior).await;
            let publisher = NatsEventPublisher::connect(&address).await.unwrap();
            let marked = Arc::new(AtomicBool::new(false));
            let queue = Queue {
                event: CommittedEvent {
                    game_id: GameId::new("test").unwrap(),
                    request_id: RequestId::new("r").unwrap(),
                    user_id: UserId::new("black").unwrap(),
                    seq: 1,
                    event: DomainEvent::MoveCommitted {
                        movement: "7g7f".parse().unwrap(),
                        in_check: false,
                        result: None,
                    },
                },
                marked: marked.clone(),
            };
            let mut job =
                tokio::spawn(
                    async move { OutboxPublisher::new(queue, publisher).publish_next().await },
                );
            tokio::time::timeout(Duration::from_secs(2), observed)
                .await
                .unwrap()
                .unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(50), &mut job)
                    .await
                    .is_err()
            );
            assert!(!marked.load(Ordering::SeqCst));
            release.send(()).unwrap();
            let outcome = tokio::time::timeout(Duration::from_secs(7), job)
                .await
                .unwrap()
                .unwrap();
            let expected = matches!(behavior, Behavior::Echo);
            assert_eq!(outcome.is_ok(), expected);
            assert_eq!(marked.load(Ordering::SeqCst), expected);
            if matches!(behavior, Behavior::Disconnect) {
                tokio::time::timeout(Duration::from_secs(2), server)
                    .await
                    .unwrap()
                    .unwrap();
            } else {
                server.abort();
            }
        }
    }
    #[tokio::test]
    async fn rejected_event_cannot_be_confirmed_by_an_unrelated_echo() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("nats://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (read, mut write) = stream.into_split();
            let mut reader = BufReader::new(read);
            write
                .write_all(b"INFO {\"server_id\":\"fake\",\"max_payload\":1048576,\"proto\":1}\r\n")
                .await
                .unwrap();
            let mut subscriptions = std::collections::HashMap::new();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).await.unwrap() == 0 {
                    break;
                }
                let fields: Vec<_> = line.split_whitespace().collect();
                match fields[0] {
                    "CONNECT" | "UNSUB" => {}
                    "PING" => {
                        write.write_all(b"PONG\r\n").await.unwrap();
                    }
                    "SUB" => {
                        subscriptions.insert(fields[1].to_owned(), fields[2].to_owned());
                    }
                    "PUB" => {
                        let size: usize = fields.last().unwrap().parse().unwrap();
                        let mut payload = vec![0; size + 2];
                        reader.read_exact(&mut payload).await.unwrap();
                        if fields[1].starts_with("shogi.games.") {
                            write
                                .write_all(
                                    format!(
                                        "-ERR 'Permissions Violation for Publish to \"{}\"'\r\n",
                                        fields[1]
                                    )
                                    .as_bytes(),
                                )
                                .await
                                .unwrap();
                            if let Some(sid) = subscriptions.get(fields[1]) {
                                write
                                    .write_all(
                                        format!("MSG {} {sid} 7\r\nbarrier\r\n", fields[1])
                                            .as_bytes(),
                                    )
                                    .await
                                    .unwrap();
                            }
                        } else {
                            let sid = subscriptions.get(fields[1]).unwrap();
                            write
                                .write_all(format!("MSG {} {sid} {size}\r\n", fields[1]).as_bytes())
                                .await
                                .unwrap();
                            write.write_all(&payload).await.unwrap();
                        }
                    }
                    other => panic!("unexpected command {other}"),
                }
            }
        });
        let publisher = NatsEventPublisher::connect(&address).await.unwrap();
        let marked = Arc::new(AtomicBool::new(false));
        let queue = Queue {
            event: CommittedEvent {
                game_id: GameId::new("test").unwrap(),
                seq: 1,
                request_id: RequestId::new("r").unwrap(),
                user_id: UserId::new("black").unwrap(),
                event: DomainEvent::MoveCommitted {
                    movement: "7g7f".parse().unwrap(),
                    in_check: false,
                    result: None,
                },
            },
            marked: marked.clone(),
        };
        let outcome = OutboxPublisher::new(queue, publisher).publish_next().await;
        assert!(
            !marked.load(Ordering::SeqCst),
            "a rejected event must remain pending even when an unrelated echo succeeds"
        );
        assert!(outcome.is_err());
        server.abort();
    }
}
