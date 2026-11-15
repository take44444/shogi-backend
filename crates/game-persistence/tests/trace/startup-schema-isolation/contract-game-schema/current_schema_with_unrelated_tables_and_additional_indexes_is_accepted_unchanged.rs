crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL compatible additional objects and unchanged snapshots.",
        reason: "Unrelated tables and additional/renamed indexes are accepted unchanged.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn current_schema_with_unrelated_tables_and_additional_indexes_is_accepted_unchanged() {
    let database = TestDatabase::new().await;
    let raw = Database::connect(database.url()).await.unwrap();
    raw.execute_unprepared(include_str!("../../../fixtures/schema-v7.sql"))
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
}
