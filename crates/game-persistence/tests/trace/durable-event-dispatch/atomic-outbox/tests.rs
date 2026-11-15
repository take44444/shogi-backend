use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL writer/outbox persistence, replay, rejection, restoration and frozen payload.",
        reason: "Real database assertions count events after replay, rejection and restoration and prove payload remains frozen after journal mutation.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "durable-event-dispatch", contract: "exclusive-queue", scope: "Real PostgreSQL writer/outbox persistence, replay, rejection, restoration and frozen payload.", relationship: crate::trace_support::Relationship::Asserts, reason: "Real PostgreSQL returns the frozen move before the sequence-two finish; marking the first row twice still leaves the finish pending." }, crate::trace_support::Evidence { feature: "game-command-recovery", contract: "canonical-events", scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport. Reads canonical persisted events; no broker delivery guarantee.", relationship: crate::trace_support::Relationship::Asserts, reason: "Persisted event payload stays frozen; retry/rejection/restoration add no duplicate events." }],
    };
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
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL trigger-injected enqueue failure and rollback.",
        reason: "An enqueue-failing trigger rolls back game metadata, journal and outbox.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "game-command-recovery", contract: "durable-commit", scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport. Injected outbox enqueue failure.", relationship: crate::trace_support::Relationship::Asserts, reason: "Outbox enqueue failure rolls back game metadata, journal and events." }],
    };
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
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL uncertain commit injection and idempotent retry.",
        reason: "Retry after an uncertain real commit yields one outbox row.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}
