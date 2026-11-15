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
