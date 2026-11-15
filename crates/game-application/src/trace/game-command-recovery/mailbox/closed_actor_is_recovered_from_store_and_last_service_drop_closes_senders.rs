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
