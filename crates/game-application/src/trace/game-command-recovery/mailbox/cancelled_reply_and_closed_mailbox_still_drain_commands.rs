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
