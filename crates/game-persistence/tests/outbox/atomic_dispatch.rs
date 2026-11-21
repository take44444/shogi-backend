use super::*;

#[traceability::targets(
    game_persistence::PostgresOutbox::next_pending,
    game_persistence::PostgresOutbox::mark_published
)]
#[tokio::test]
async fn retry_rejection_restoration_and_frozen_payload() {
    let database = TestDatabase::new().await;
    let url = database.url();
    let (store, service, raw) = setup(url).await;
    assert_eq!(count(&raw, "outbox_events").await, 0);
    assert!(
        service
            .make_move(
                &id(),
                MakeMove {
                    user_id: user("white"),
                    ..command()
                }
            )
            .await
            .is_err()
    );
    assert_eq!(count(&raw, "outbox_events").await, 0);
    let accepted = service.make_move(&id(), command()).await.unwrap();
    assert_eq!(service.make_move(&id(), command()).await.unwrap(), accepted);
    let restored = pg_test_support::GameFixture::new(store.clone(), database.url());
    assert_eq!(restored.snapshot(&id()).await.unwrap().seq, 1);
    assert_eq!(
        restored.make_move(&id(), command()).await.unwrap(),
        accepted
    );
    assert_eq!(count(&raw, "outbox_events").await, 1);
    service
        .resign(
            &id(),
            Resign {
                request_id: RequestId::new("end").unwrap(),
                expected_seq: 1,
                user_id: user("white"),
            },
        )
        .await
        .unwrap();
    assert_eq!(count(&raw, "outbox_events").await, 2);
    raw.execute_unprepared("UPDATE journal SET usi = '2g2f' WHERE seq = 1")
        .await
        .unwrap();
    let mut queue = PostgresOutbox::connect(url).await.unwrap();
    let item = queue.next_pending().await.unwrap().unwrap();
    assert!(
        matches!(item.event.event, DomainEvent::MoveCommitted { movement, .. } if movement.to_string() == "7g7f")
    );
    queue.mark_published(&item).await.unwrap();
    queue.mark_published(&item).await.unwrap();
    let end = queue.next_pending().await.unwrap().unwrap();
    assert_eq!(end.event.seq, 2);
    assert!(matches!(end.event.event, DomainEvent::GameFinished { .. }));
    queue.mark_published(&end).await.unwrap();
    assert!(queue.next_pending().await.unwrap().is_none());
}

#[traceability::targets(game_application::GameService::make_move)]
#[tokio::test]
async fn failed_enqueue_rolls_back_metadata_journal_and_events() {
    let database = TestDatabase::new().await;
    let url = database.url();
    let (store, service, raw) = setup(url).await;
    raw.execute_unprepared("CREATE FUNCTION fail_outbox() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected outbox failure'; END $$; CREATE TRIGGER fail_outbox BEFORE INSERT ON outbox_events FOR EACH ROW EXECUTE FUNCTION fail_outbox()").await.unwrap();
    assert_eq!(
        service.make_move(&id(), command()).await,
        Err(Error::Unavailable)
    );
    assert_eq!(store.version(&id()).await.unwrap(), 0);
    assert_eq!(count(&raw, "journal").await, 0);
    assert_eq!(count(&raw, "outbox_events").await, 0);
    raw.execute_unprepared("DROP TRIGGER fail_outbox ON outbox_events")
        .await
        .unwrap();
    assert_eq!(service.make_move(&id(), command()).await.unwrap().seq, 1);
    assert_eq!(count(&raw, "outbox_events").await, 1);
}

#[traceability::targets(game_application::GameService::make_move)]
#[tokio::test]
async fn uncertain_commit_and_retry_enqueue_once() {
    let database = TestDatabase::new().await;
    let url = database.url();
    let (store, _, raw) = setup(url).await;
    let service = pg_test_support::GameFixture::new(Arc::new(Uncertain(store)), database.url());
    let accepted = service.make_move(&id(), command()).await.unwrap();
    assert_eq!(service.make_move(&id(), command()).await.unwrap(), accepted);
    assert_eq!(count(&raw, "outbox_events").await, 1);
}
