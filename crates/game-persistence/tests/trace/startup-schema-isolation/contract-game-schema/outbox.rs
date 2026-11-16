use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL writer startup rejects selected malformed ledgers and orphaned outbox schema.",
        reason: "Malformed version-ledger definitions and an orphaned outbox table cause writer startup to return Corrupt.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn malformed_ledgers_and_orphaned_outbox_tables_fail_closed() {
    for ddl in [
        "CREATE TABLE schema_version (id INTEGER); INSERT INTO schema_version VALUES (0)",
        "CREATE TABLE schema_version (id INTEGER, version TEXT); INSERT INTO schema_version VALUES (0, 'invalid')",
        "CREATE TABLE outbox_events (id INTEGER)",
    ] {
        let database = TestDatabase::new().await;
        let url = database.url();
        let raw = Database::connect(url).await.unwrap();
        raw.execute_unprepared(ddl).await.unwrap();
        assert!(
            matches!(SeaOrmStore::connect(url).await, Err(Error::Corrupt)),
            "{ddl}"
        );
    }
}
}
