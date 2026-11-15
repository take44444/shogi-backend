crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL fresh schema catalog compared with the current fixture.",
        reason: "A newly built schema matches the current schema fixture catalog.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn fresh_builder_schema_matches_the_current_schema_fixture() {
    let previous = pg_test_support::TestDatabase::new().await;
    let previous_db = Database::connect(previous.url()).await.unwrap();
    previous_db
        .execute_unprepared(include_str!("../../../fixtures/schema-v7.sql"))
        .await
        .unwrap();
    let current = pg_test_support::TestDatabase::new().await;
    let store = SeaOrmStore::connect(current.url()).await.unwrap();
    let current_db = Database::connect(current.url()).await.unwrap();
    let catalog = "SELECT definition FROM (
        SELECT 'column|' || json_build_array(table_name, ordinal_position, column_name, data_type, is_nullable, column_default, collation_name, is_identity, identity_generation)::text AS definition
        FROM information_schema.columns WHERE table_schema = 'public'
        UNION ALL
        SELECT 'constraint|' || json_build_array(c.relname, k.conname, pg_get_constraintdef(k.oid), k.condeferrable, k.condeferred, k.convalidated)::text
        FROM pg_constraint k JOIN pg_class c ON c.oid = k.conrelid JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public'
        UNION ALL
        SELECT 'index|' || indexdef FROM pg_indexes WHERE schemaname = 'public'
    ) definitions ORDER BY definition";
    let mut definitions = Vec::new();
    for db in [&previous_db, &current_db] {
        definitions.push(
            db.query_all_raw(Statement::from_string(DbBackend::Postgres, catalog))
                .await
                .unwrap()
                .into_iter()
                .map(|row| row.try_get::<String>("", "definition").unwrap())
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(definitions[0], definitions[1]);
    store.close().await.unwrap();
}
}
