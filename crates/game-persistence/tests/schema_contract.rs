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

include!(
    "trace/startup-schema-isolation/contract-game-schema/changed_required_schema_is_rejected_without_modifying_schema_or_data.rs"
);

include!(
    "trace/startup-schema-isolation/contract-game-schema/current_schema_with_unrelated_tables_and_additional_indexes_is_accepted_unchanged.rs"
);

include!(
    "trace/startup-schema-isolation/contract-game-schema/version_five_is_rejected_without_migration_or_deletion.rs"
);

include!(
    "trace/startup-schema-isolation/contract-game-schema/version_six_is_rejected_unchanged.rs"
);

#[cfg(test)]
#[path = "../../../scripts/traceability_model.rs"]
mod trace_support;
