#![cfg(test)]

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

#[path = "../confirmed-dispatch/tests.rs"]
mod confirmed_dispatch;
