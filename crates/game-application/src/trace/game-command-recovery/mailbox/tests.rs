use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Actor/fake repository and channel lifecycle; no process shutdown signal.",
        reason: "Dropped response and closed mailbox still execute queued commands.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn cancelled_reply_and_closed_mailbox_still_drain_commands() {
    let actor = Actor::new(id(), store());
    let (sender, receiver) = mpsc::channel(MAILBOX_CAPACITY);
    let (reply, response) = oneshot::channel();
    sender
        .send(Command::Mutate {
            receipt: receipt(),
            reply,
        })
        .await
        .unwrap();
    drop(response);
    let (reply, response) = oneshot::channel();
    sender
        .send(Command::Mutate {
            receipt: receipt(),
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
    assert_eq!(snapshot_response.await.unwrap().unwrap().seq, 1);
    task.await.unwrap();
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Controlled fake repository/channel capacity boundary.",
        reason: "A capacity-blocked cancelled send does not execute while an enqueued cancelled call does.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn cancelled_call_after_enqueue_executes_and_before_enqueue_does_not() {
    for full in [false, true] {
        let store = store();
        let service = GameService::new(store.clone());
        let (sender, receiver) = mpsc::channel(MAILBOX_CAPACITY);
        service.games.lock().unwrap().insert(id(), sender.clone());
        if full {
            for _ in 0..MAILBOX_CAPACITY {
                let (reply, _) = oneshot::channel();
                sender.send(Command::Snapshot(reply)).await.unwrap();
            }
        }
        let game_id = id();
        let command = MakeMove {
            request_id: receipt().request_id,
            user_id: receipt().user_id,
            expected_seq: 0,
            movement: "7g7f".parse().unwrap(),
        };
        let mut call = Box::pin(service.make_move(&game_id, command.clone()));
        tokio::select! { biased; result = &mut call => panic!("actor has not started, got {result:?}"), _ = tokio::task::yield_now() => {} }
        assert_eq!(receiver.len(), if full { MAILBOX_CAPACITY } else { 1 });
        drop(call);
        let task = tokio::spawn(Actor::new(id(), store).run(receiver));
        assert_eq!(service.snapshot(&id()).await.unwrap().seq, u64::from(!full));
        assert_eq!(service.make_move(&id(), command).await.unwrap().seq, 1);
        drop(sender);
        drop(service);
        task.await.unwrap();
    }
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "In-memory RecordStore and sender lifecycle, not process restart.",
        reason: "Closed actor reloads durable store and final service drop closes senders.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn closed_actor_is_recovered_from_store_and_last_service_drop_closes_senders() {
    let service = GameService::new(store());
    let (sender, receiver) = mpsc::channel(MAILBOX_CAPACITY);
    drop(receiver);
    service.games.lock().unwrap().insert(id(), sender);
    assert_eq!(service.snapshot(&id()).await.unwrap().seq, 0);
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
    drop(clone);
    assert!(weak.upgrade().is_none());
}
}
