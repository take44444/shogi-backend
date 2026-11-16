use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL で、選択した不正な台帳と、他の必要な表がない Outbox スキーマを、書き込み側の起動時に拒否することを検証する。",
        reason: "不正なバージョン台帳の定義や、他の必要な表がない Outbox テーブルにより、書き込みプロセスの起動は Corrupt を返す。",
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
