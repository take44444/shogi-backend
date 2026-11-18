use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-record-persistence/reader-database", relationship: crate::trace_support::Relationship::Asserts, scope: "実 PostgreSQL と実保存アダプターの公開 API または DB 接続を使用する。", reason: "対応しないスキーマを変更しないこと、または保存時の不整合拒否と読み取り結果を直接検査する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["postgres"], fake: &[] }] };
#[tokio::test]
async fn reader_does_not_create_or_accept_wrong_schema() {
    let empty = pg_test_support::TestDatabase::new().await;
    assert!(matches!(
        PostgresGameRecords::connect(empty.url()).await,
        Err(Error::Corrupt)
    ));
    let raw = Database::connect(empty.url()).await.unwrap();
    let tables = raw
        .query_all_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT table_name FROM information_schema.tables WHERE table_schema='public'",
        ))
        .await
        .unwrap();
    assert!(tables.is_empty());
    for sql in [
        "UPDATE schema_version SET version=3",
        "UPDATE schema_version SET version=99",
        "DELETE FROM schema_version",
        "DROP TABLE journal CASCADE",
        "ALTER TABLE games RENAME COLUMN finished_at TO missing",
    ] {
        let f = Fixture::new().await;
        f.raw.execute_unprepared(sql).await.unwrap();
        assert!(
            matches!(
                PostgresGameRecords::connect(&f.reader_url).await,
                Err(Error::Corrupt)
            ),
            "{sql}"
        );
    }
}
}
