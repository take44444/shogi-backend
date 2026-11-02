use super::*;
use crate::{GameService, MAILBOX_CAPACITY, MakeMove};

fn id() -> GameId {
    GameId::new("lifecycle").unwrap()
}
fn request() -> RequestId {
    RequestId::new("request").unwrap()
}
fn mutation() -> Mutation {
    Mutation::Move {
        expected_seq: 0,
        player: Player::Black,
        movement: "7g7f".parse().unwrap(),
    }
}

#[tokio::test]
async fn cancelled_reply_still_executes_and_identical_retry_recovers_receipt() {
    let actor = Actor::new(id());
    let (sender, receiver) = mpsc::channel(64);
    let (reply, response) = oneshot::channel();
    sender
        .send(Command::Mutate {
            request_id: request(),
            mutation: mutation(),
            reply,
        })
        .await
        .unwrap();
    drop(response);
    let task = tokio::spawn(actor.run(receiver));
    let (reply, response) = oneshot::channel();
    sender
        .send(Command::Mutate {
            request_id: request(),
            mutation: mutation(),
            reply,
        })
        .await
        .unwrap();
    let receipt = response.await.unwrap().unwrap();
    assert_eq!(receipt.seq, 1);
    let (reply, response) = oneshot::channel();
    sender.send(Command::Snapshot(reply)).await.unwrap();
    let snapshot = response.await.unwrap();
    assert_eq!(snapshot.seq, 1);
    assert_eq!(snapshot.turn, Player::White);
    drop(sender);
    task.await.unwrap();
}

#[tokio::test]
async fn closed_mailbox_drains_enqueued_commands_then_actor_exits() {
    let actor = Actor::new(id());
    let (sender, receiver) = mpsc::channel(64);
    let (reply, response) = oneshot::channel();
    sender
        .send(Command::Mutate {
            request_id: request(),
            mutation: mutation(),
            reply,
        })
        .await
        .unwrap();
    let (snapshot_reply, snapshot_response) = oneshot::channel();
    sender
        .send(Command::Snapshot(snapshot_reply))
        .await
        .unwrap();
    drop(sender);
    let task = tokio::spawn(actor.run(receiver));
    assert_eq!(response.await.unwrap().unwrap().seq, 1);
    assert_eq!(snapshot_response.await.unwrap().seq, 1);
    task.await.unwrap();
}

#[tokio::test]
async fn last_service_drop_closes_actor_sender_but_clone_keeps_it_alive() {
    let service = GameService::new();
    service.create(id()).await.unwrap();
    let weak = service
        .games
        .lock()
        .unwrap()
        .get(&id())
        .unwrap()
        .downgrade();
    let clone = service.clone();
    drop(service);
    assert!(weak.upgrade().is_some());
    assert_eq!(clone.snapshot(&id()).await.unwrap().seq, 0);
    drop(clone);
    assert!(weak.upgrade().is_none());
}

#[tokio::test]
async fn lost_actor_returns_unavailable_without_recreation() {
    let service = GameService::new();
    let (sender, receiver) = mpsc::channel(64);
    drop(receiver);
    service.games.lock().unwrap().insert(id(), sender);
    assert_eq!(
        service.snapshot(&id()).await,
        Err(ApplicationError::Unavailable)
    );
    assert_eq!(
        service
            .make_move(
                &id(),
                MakeMove {
                    request_id: request(),
                    expected_seq: 0,
                    player: Player::Black,
                    movement: "7g7f".parse().unwrap(),
                }
            )
            .await,
        Err(ApplicationError::Unavailable)
    );
    assert_eq!(
        service.create(id()).await,
        Err(ApplicationError::AlreadyExists)
    );
}

#[tokio::test]
async fn full_mailbox_waits_without_blocking_a_different_game() {
    let service = GameService::new();
    let (sender, receiver) = mpsc::channel(MAILBOX_CAPACITY);
    service.games.lock().unwrap().insert(id(), sender.clone());
    for _ in 0..MAILBOX_CAPACITY {
        let (reply, _) = oneshot::channel();
        sender.send(Command::Snapshot(reply)).await.unwrap();
    }
    let full_game = id();
    let mut blocked = Box::pin(service.snapshot(&full_game));
    tokio::select! {
        biased;
        result = &mut blocked => panic!("full mailbox returned {result:?}"),
        _ = tokio::task::yield_now() => {}
    }
    let other = GameId::new("other").unwrap();
    assert_eq!(service.create(other.clone()).await.unwrap().seq, 0);
    assert_eq!(service.snapshot(&other).await.unwrap().seq, 0);
    let task = tokio::spawn(Actor::new(id()).run(receiver));
    assert_eq!(blocked.await.unwrap().seq, 0);
    drop(sender);
    drop(service);
    task.await.unwrap();
}

#[test]
fn sequence_exhaustion_precedes_mutation_and_does_not_reserve_request_id() {
    let mut actor = Actor::new(id());
    actor.seq = u64::MAX;
    let before = actor.snapshot();
    for mutation in [
        Mutation::Move {
            expected_seq: u64::MAX,
            player: Player::Black,
            movement: "7g7f".parse().unwrap(),
        },
        Mutation::Resign {
            expected_seq: u64::MAX,
            player: Player::White,
        },
    ] {
        assert_eq!(
            actor.apply(request(), mutation),
            Err(ApplicationError::SequenceExhausted)
        );
        assert_eq!(actor.snapshot(), before);
        assert!(actor.accepted.is_empty());
    }
}

#[tokio::test]
async fn cancelling_application_call_after_enqueue_can_be_retried_exactly() {
    let service = GameService::new();
    let game_id = id();
    let (sender, receiver) = mpsc::channel(MAILBOX_CAPACITY);
    service
        .games
        .lock()
        .unwrap()
        .insert(game_id.clone(), sender);
    let command = MakeMove {
        request_id: request(),
        expected_seq: 0,
        player: Player::Black,
        movement: "7g7f".parse().unwrap(),
    };
    let mut call = Box::pin(service.make_move(&game_id, command.clone()));
    tokio::select! {
        biased;
        result = &mut call => panic!("actor has not started, got {result:?}"),
        _ = tokio::task::yield_now() => {}
    }
    assert_eq!(receiver.len(), 1);
    drop(call);
    let task = tokio::spawn(Actor::new(game_id.clone()).run(receiver));
    assert_eq!(service.make_move(&game_id, command).await.unwrap().seq, 1);
    let snapshot = service.snapshot(&game_id).await.unwrap();
    assert_eq!(snapshot.seq, 1);
    assert_eq!(snapshot.turn, Player::White);
    drop(service);
    task.await.unwrap();
}

#[tokio::test]
async fn cancelling_application_call_before_enqueue_does_not_execute_it() {
    let service = GameService::new();
    let game_id = id();
    let (sender, receiver) = mpsc::channel(MAILBOX_CAPACITY);
    service
        .games
        .lock()
        .unwrap()
        .insert(game_id.clone(), sender.clone());
    for _ in 0..MAILBOX_CAPACITY {
        let (reply, _) = oneshot::channel();
        sender.send(Command::Snapshot(reply)).await.unwrap();
    }
    let mut call = Box::pin(service.make_move(
        &game_id,
        MakeMove {
            request_id: request(),
            expected_seq: 0,
            player: Player::Black,
            movement: "7g7f".parse().unwrap(),
        },
    ));
    tokio::select! {
        biased;
        result = &mut call => panic!("full mailbox returned {result:?}"),
        _ = tokio::task::yield_now() => {}
    }
    drop(call);
    let task = tokio::spawn(Actor::new(game_id.clone()).run(receiver));
    let snapshot = service.snapshot(&game_id).await.unwrap();
    assert_eq!(snapshot.seq, 0);
    assert_eq!(snapshot.turn, Player::Black);
    drop(sender);
    drop(service);
    task.await.unwrap();
}
