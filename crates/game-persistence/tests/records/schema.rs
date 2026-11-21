use super::*;

#[traceability::targets(
    game_persistence::SeaOrmStore::connect,
    game_record_persistence::PostgresGameRecords::connect
)]
#[tokio::test]
async fn current_schema_remains_readable_and_writable_with_query_builders() {
    let database = pg_test_support::TestDatabase::new().await;
    let raw = Database::connect(database.url()).await.unwrap();
    raw.execute_unprepared(include_str!("../fixtures/schema-v8.sql"))
        .await
        .unwrap();
    let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
    let games = pg_test_support::GameFixture::new(store.clone(), database.url());
    let id = WriteId::new("existing-schema").unwrap();
    games
        .seed(
            id.clone(),
            Participants::new(UserId::new("black").unwrap(), UserId::new("white").unwrap())
                .unwrap(),
        )
        .await
        .unwrap();
    games
        .resign(
            &id,
            Resign {
                request_id: RequestId::new("end").unwrap(),
                expected_seq: 0,
                user_id: UserId::new("white").unwrap(),
            },
        )
        .await
        .unwrap();
    let reader = PostgresGameRecords::connect(&database.reader_url().await)
        .await
        .unwrap();
    let detail = reader
        .detail(&GameId::new(id.as_str()).unwrap())
        .await
        .unwrap();
    assert_eq!(detail.metadata.final_seq().get(), 1);
    assert_eq!(detail.history[0].action, PublicAction::Resign);
    reader.close().await;
    store.close().await.unwrap();
}

#[traceability::targets(game_persistence::SeaOrmStore::connect)]
#[tokio::test]
async fn fresh_builder_schema_matches_the_current_schema_fixture() {
    let previous = pg_test_support::TestDatabase::new().await;
    let previous_db = Database::connect(previous.url()).await.unwrap();
    previous_db
        .execute_unprepared(include_str!("../fixtures/schema-v8.sql"))
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
