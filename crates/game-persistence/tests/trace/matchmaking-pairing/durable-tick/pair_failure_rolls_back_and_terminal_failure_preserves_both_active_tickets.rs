crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport.",
        reason: "Injected slot failure leaves zero game/rated/slot rows and both waiting; successful retry freezes original Unrated states.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "admission", scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport. Injected SQL failures and post-finish rating reservation checks.", relationship: crate::trace_support::Relationship::Asserts, reason: "After game finish the same-pool admission is rating_pending, while another pool remains available." }],
    };
#[tokio::test]
async fn pair_failure_rolls_back_and_terminal_failure_preserves_both_active_tickets() {
    let f = Fixture::new().await;
    let one = f.join("one", POOL).await;
    let two = f.join("two", POOL).await;
    f.raw.execute_unprepared("CREATE FUNCTION reject_slot() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected slot failure'; END $$; CREATE TRIGGER reject_slot BEFORE INSERT ON rating_slots FOR EACH ROW EXECUTE FUNCTION reject_slot()").await.unwrap();
    assert_eq!(f.store.tick(POOL).await, Err(MatchmakingError::Unavailable));
    for table in ["games", "rated_games", "rating_slots"] {
        assert_eq!(f.count(table).await, 0);
    }
    for (user, id) in [("one", one), ("two", two)] {
        assert!(matches!(
            f.store.ticket(&uid(user), id).await.unwrap().state,
            TicketState::Waiting(_)
        ));
    }
    f.raw
        .execute_unprepared("DROP TRIGGER reject_slot ON rating_slots")
        .await
        .unwrap();
    let game = f.store.tick(POOL).await.unwrap().unwrap();
    let identity = pg_test_support::rated_identity(f._db.url(), &game)
        .await
        .unwrap();
    assert_eq!(identity.black_start, RatingState::Unrated);
    assert_eq!(identity.white_start, RatingState::Unrated);
    f.raw.execute_unprepared("CREATE TRIGGER reject_job BEFORE INSERT ON rating_jobs FOR EACH ROW EXECUTE FUNCTION reject_slot()").await.unwrap();
    let service = GameService::new(f.store.clone());
    let command = Resign {
        request_id: RequestId::new("finish").unwrap(),
        user_id: uid("one"),
        expected_seq: 0,
    };
    assert!(service.resign(&game, command.clone()).await.is_err());
    assert_eq!(f.store.version(&game).await.unwrap(), 0);
    for table in ["journal", "rating_jobs", "outbox_events"] {
        assert_eq!(f.count(table).await, 0);
    }
    for (user, id) in [("one", one), ("two", two)] {
        assert!(matches!(
            f.store.ticket(&uid(user), id).await.unwrap().state,
            TicketState::Playing { .. }
        ));
    }
    f.raw
        .execute_unprepared("DROP TRIGGER reject_job ON rating_jobs")
        .await
        .unwrap();
    let result = service.resign(&game, command.clone()).await.unwrap();
    assert_eq!(service.resign(&game, command).await.unwrap(), result);
    assert_eq!(
        f.store.current(&uid("one")).await.unwrap(),
        Participation::Idle
    );
    assert_eq!(f.count("rating_slots").await, 2);
    assert_eq!(
        f.store
            .join(uid("one"), TicketId::new(), POOL)
            .await
            .unwrap()
            .ticket
            .state,
        TicketState::Refused(RefusalReason::RatingPending)
    );
    let next = f.join("one", Pool::Byoyomi30).await;
    f.store.cancel(&uid("one"), one).await.unwrap();
    assert!(matches!(
        f.store.ticket(&uid("one"), next).await.unwrap().state,
        TicketState::Waiting(_)
    ));
}
}
