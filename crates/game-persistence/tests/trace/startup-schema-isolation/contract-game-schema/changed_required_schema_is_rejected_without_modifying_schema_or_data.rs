crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL fifteen required game-schema mutations with before/after snapshots.",
        reason: "Fifteen required-definition mutations are rejected while before/after schema and data snapshots remain equal.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn changed_required_schema_is_rejected_without_modifying_schema_or_data() {
    for change in [
        "ALTER TABLE games ALTER COLUMN rule DROP NOT NULL",
        "ALTER TABLE journal DROP COLUMN user_id",
        "ALTER TABLE games ALTER COLUMN rule TYPE VARCHAR",
        "ALTER TABLE outbox_events DROP CONSTRAINT outbox_events_game_id_seq_event_kind_key",
        "ALTER TABLE journal DROP CONSTRAINT journal_game_id_fkey",
        "ALTER TABLE games DROP CONSTRAINT games_rule_check",
        "ALTER TABLE outbox_events ALTER COLUMN id DROP IDENTITY",
        "DROP INDEX records_black",
        "DROP INDEX rating_jobs_pending",
        "ALTER TABLE rating_states DROP CONSTRAINT rating_states_check",
        "ALTER TABLE rating_slots DROP CONSTRAINT rating_slots_pkey",
        "ALTER TABLE rating_jobs ALTER COLUMN generation DROP NOT NULL",
        "ALTER TABLE rating_applications DROP COLUMN delta_black",
        "DROP INDEX matchmaking_active_user",
        "ALTER TABLE matchmaking_tickets DROP COLUMN heartbeat_seq",
    ] {
        let database = TestDatabase::new().await;
        SeaOrmStore::connect(database.url())
            .await
            .unwrap()
            .close()
            .await
            .unwrap();
        let raw = Database::connect(database.url()).await.unwrap();
        raw.execute_unprepared(
            "INSERT INTO games (id, black, white, version, rule, visibility, final_move_ply)
                VALUES ('preserved', 'black', 'white', 1, 'standard', 'public', 1);
             INSERT INTO journal (game_id, seq, request_id, user_id, expected_seq, action, usi, in_check)
                VALUES ('preserved', 1, 'first', 'black', 0, 1, '7g7f', FALSE);
             INSERT INTO outbox_events (game_id, seq, event_kind, encoding_version, request_id, user_id, usi, in_check)
                VALUES ('preserved', 1, 0, 1, 'first', 'black', '7g7f', FALSE)",
        )
        .await
        .unwrap();
        raw.execute_unprepared(change).await.unwrap();
        let before = snapshot(&raw).await;
        let result = SeaOrmStore::connect(database.url()).await.map(|_| ());
        assert_eq!(snapshot(&raw).await, before, "{change}");
        assert_eq!(result, Err(Error::Corrupt), "{change}");
        raw.close().await.unwrap();
    }
}
}
