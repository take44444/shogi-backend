use game_application::ApplicationError as Error;
use game_persistence::SeaOrmStore;
use pg_test_support::TestDatabase;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};

async fn snapshot(db: &DatabaseConnection) -> Vec<String> {
    db.query_all_raw(Statement::from_string(
        DbBackend::Postgres,
        "SELECT definition FROM (
            SELECT 'namespace|' || nspname AS definition FROM pg_namespace
                WHERE nspname NOT LIKE 'pg_%' AND nspname <> 'information_schema'
            UNION ALL
            SELECT 'column|' || row_to_json(c)::text FROM information_schema.columns c
                WHERE table_schema = current_schema()
            UNION ALL
            SELECT 'constraint|' || c.relname || '|' || k.conname || '|' || pg_get_constraintdef(k.oid)
                FROM pg_constraint k JOIN pg_class c ON c.oid = k.conrelid
                JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = current_schema()
            UNION ALL
            SELECT 'index|' || indexdef FROM pg_indexes WHERE schemaname = current_schema()
            UNION ALL
            SELECT 'version|' || row_to_json(t)::text FROM schema_version t
            UNION ALL
            SELECT 'game|' || row_to_json(t)::text FROM games t
            UNION ALL
            SELECT 'journal|' || row_to_json(t)::text FROM journal t
            UNION ALL
            SELECT 'outbox|' || row_to_json(t)::text FROM outbox_events t
        ) definitions ORDER BY definition",
    ))
    .await
    .unwrap()
    .into_iter()
    .map(|row| row.try_get("", "definition").unwrap())
    .collect()
}

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

#[tokio::test]
async fn current_schema_with_unrelated_tables_and_additional_indexes_is_accepted_unchanged() {
    let database = TestDatabase::new().await;
    let raw = Database::connect(database.url()).await.unwrap();
    raw.execute_unprepared(include_str!("fixtures/schema-v7.sql"))
        .await
        .unwrap();
    raw.execute_unprepared(
        "CREATE TABLE application_extra (value TEXT);
         INSERT INTO application_extra VALUES ('preserved');
         CREATE INDEX application_extra_value ON application_extra(value);
         CREATE INDEX games_white_extra ON games(white);
         ALTER INDEX records_black RENAME TO renamed_records_black;
         ALTER TABLE journal RENAME CONSTRAINT journal_game_id_fkey TO renamed_journal_fk;
         ALTER TABLE games ADD COLUMN application_extra TEXT",
    )
    .await
    .unwrap();
    let before = snapshot(&raw).await;
    SeaOrmStore::connect(database.url())
        .await
        .unwrap()
        .close()
        .await
        .unwrap();
    assert_eq!(snapshot(&raw).await, before);
    raw.close().await.unwrap();
}

#[tokio::test]
async fn version_five_is_rejected_without_migration_or_deletion() {
    let database = TestDatabase::new().await;
    let raw = Database::connect(database.url()).await.unwrap();
    raw.execute_unprepared(include_str!("fixtures/schema-v5.sql"))
        .await
        .unwrap();
    let before = snapshot(&raw).await;
    assert_eq!(
        SeaOrmStore::connect(database.url()).await.map(|_| ()),
        Err(Error::Corrupt)
    );
    assert_eq!(snapshot(&raw).await, before);
    raw.close().await.unwrap();
}

#[tokio::test]
async fn version_six_is_rejected_unchanged() {
    let database = TestDatabase::new().await;
    let raw = Database::connect(database.url()).await.unwrap();
    raw.execute_unprepared(include_str!("fixtures/schema-v6.sql"))
        .await
        .unwrap();
    let before = snapshot(&raw).await;
    assert_eq!(
        SeaOrmStore::connect(database.url()).await.map(|_| ()),
        Err(Error::Corrupt)
    );
    assert_eq!(snapshot(&raw).await, before);
}
