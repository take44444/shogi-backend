use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL lock owner and spawned lock-probe process; registers the driver rather than the inert helper.",
        reason: "A child process cannot acquire the publisher lock until the owner drops it.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
    async fn publisher_session_lock_excludes_a_second_process_and_releases_on_drop() {
        let database = TestDatabase::new().await;
        let url = database.url();
        let queue = PostgresOutbox::connect(url).await.unwrap();
        assert!(matches!(
            PostgresOutbox::connect(url).await,
            Err(game_persistence::OutboxOpenError::PublisherAlreadyRunning)
        ));
        assert!(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "publisher_lock_probe", "--nocapture"])
                .env("OUTBOX_LOCK_PROBE_URL", url)
                .env("OUTBOX_LOCK_PROBE_HELD", "1")
                .status()
                .unwrap()
                .success()
        );
        drop(queue);
        assert!(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "publisher_lock_probe", "--nocapture"])
                .env("OUTBOX_LOCK_PROBE_URL", url)
                .env("OUTBOX_LOCK_PROBE_HELD", "0")
                .status()
                .unwrap()
                .success()
        );
    }
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL delivery-boundary observation with a test publisher.",
        reason: "At the delivery boundary PostgreSQL reports no open transaction and another writer can commit.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
    async fn publisher_holds_no_transaction_while_awaiting_delivery_and_writes_continue() {
        let database = TestDatabase::new().await;
        let (_, service, raw) = setup(database.url()).await;
        let mut queue = PostgresOutbox::connect(database.url()).await.unwrap();
        service.make_move(&id(), command()).await.unwrap();
        let pending = queue.next_pending().await.unwrap().unwrap();
        let open_transactions: i64 = raw.query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT count(*) AS count FROM pg_stat_activity WHERE datname = current_database() AND application_name = 'shogi-outbox-publisher' AND xact_start IS NOT NULL",
        )).await.unwrap().unwrap().try_get("", "count").unwrap();
        assert_eq!(open_transactions, 0);
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
        queue.mark_published(&pending).await.unwrap();
        assert_eq!(queue.next_pending().await.unwrap().unwrap().event.seq, 2);
    }
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL session termination and replacement queue.",
        reason: "Terminating the owning database session causes the old queue to fail; a replacement resumes the same pending event.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
    async fn lost_publisher_session_fails_closed_and_pending_work_can_be_reopened() {
        let database = TestDatabase::new().await;
        let (_, service, raw) = setup(database.url()).await;
        service.make_move(&id(), command()).await.unwrap();
        let mut queue = PostgresOutbox::connect(database.url()).await.unwrap();
        let pending = queue.next_pending().await.unwrap().unwrap();
        raw.query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT pg_terminate_backend(pid) AS terminated FROM pg_stat_activity WHERE datname = current_database() AND application_name = 'shogi-outbox-publisher'",
        )).await.unwrap().unwrap();
        assert!(matches!(
            queue.next_pending().await,
            Err(Error::Unavailable)
        ));
        assert!(matches!(
            queue.mark_published(&pending).await,
            Err(Error::Unavailable)
        ));
        let mut replacement = PostgresOutbox::connect(database.url()).await.unwrap();
        let restored = replacement.next_pending().await.unwrap().unwrap();
        assert_eq!(restored.event, pending.event);
        replacement.mark_published(&restored).await.unwrap();
        assert!(replacement.next_pending().await.unwrap().is_none());
    }
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL pending-event decoding rejects an unsupported encoding version.",
        reason: "Changing a persisted event encoding version to 99 makes next_pending return Corrupt.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
    async fn unsupported_frozen_encoding_fails_closed() {
        let database = TestDatabase::new().await;
        let url = database.url();
        let (_, service, raw) = setup(url).await;
        service.make_move(&id(), command()).await.unwrap();
        raw.execute_unprepared("UPDATE outbox_events SET encoding_version = 99")
            .await
            .unwrap();
        let mut queue = PostgresOutbox::connect(url).await.unwrap();
        assert!(matches!(queue.next_pending().await, Err(Error::Corrupt)));
    }
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL frozen-event identifier preservation and exact-row marking.",
        reason: "A real queue round trip preserves game, request and participant identifiers containing NUL and backslashes.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
    async fn frozen_events_preserve_identifiers_with_nul_and_backslashes() {
        let database = TestDatabase::new().await;
        let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
        let service = pg_test_support::GameFixture::new(store, database.url());
        let game = GameId::new("game\0\\name").unwrap();
        let black = user("black\0\\name");
        let request = RequestId::new("request\0\\name").unwrap();
        service
            .seed(
                game.clone(),
                Participants::new(black.clone(), user("white")).unwrap(),
            )
            .await
            .unwrap();
        service
            .make_move(
                &game,
                MakeMove {
                    request_id: request.clone(),
                    expected_seq: 0,
                    user_id: black.clone(),
                    movement: "7g7f".parse().unwrap(),
                },
            )
            .await
            .unwrap();
        let mut queue = PostgresOutbox::connect(database.url()).await.unwrap();
        let pending = queue.next_pending().await.unwrap().unwrap();
        assert_eq!(pending.event.game_id, game);
        assert_eq!(pending.event.user_id, black);
        assert_eq!(pending.event.request_id, request);
        queue.mark_published(&pending).await.unwrap();
        assert!(queue.next_pending().await.unwrap().is_none());
    }
}
