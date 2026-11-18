use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-persistence/writer-database", relationship: crate::trace_support::Relationship::Asserts, scope: "実 PostgreSQL と実保存アダプターの公開 API または DB 接続を使用する。", reason: "対応しないスキーマを変更しないこと、または保存時の不整合拒否と読み取り結果を直接検査する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["postgres"], fake: &[] }] };
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn schema_bootstrap_is_serialized_and_future_versions_are_rejected() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..6 {
        let url = url.to_owned();
        tasks.spawn(async move { SeaOrmStore::connect(&url).await });
    }
    while let Some(result) = tasks.join_next().await {
        result.unwrap().unwrap();
    }
    let raw = Database::connect(&url).await.unwrap();
    raw.execute_unprepared("UPDATE schema_version SET version = 99")
        .await
        .unwrap();
    assert!(matches!(
        SeaOrmStore::connect(&url).await,
        Err(Error::Corrupt)
    ));
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-persistence/writer-database", relationship: crate::trace_support::Relationship::Asserts, scope: "実 PostgreSQL と実保存アダプターの公開 API または DB 接続を使用する。", reason: "対応しないスキーマを変更しないこと、または保存時の不整合拒否と読み取り結果を直接検査する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["postgres"], fake: &[] }] };
#[tokio::test]
async fn an_existing_empty_schema_ledger_is_rejected_without_modification() {
    for existing_tables in [false, true] {
        let database = TestDatabase::new().await;
        let url = database.url().to_string();
        if existing_tables {
            SeaOrmStore::connect(&url)
                .await
                .unwrap()
                .close()
                .await
                .unwrap();
        }
        let raw = Database::connect(&url).await.unwrap();
        if existing_tables {
            raw.execute_unprepared("DELETE FROM schema_version")
                .await
                .unwrap();
        } else {
            raw.execute_unprepared("CREATE TABLE schema_version (id INTEGER PRIMARY KEY NOT NULL, version INTEGER NOT NULL)").await.unwrap();
        }
        let actual = SeaOrmStore::connect(&url).await.map(|_| ());
        assert_eq!(
            actual,
            Err(Error::Corrupt),
            "existing_tables={existing_tables}"
        );
        let row = raw
            .query_one_raw(sea_orm::Statement::from_string(
                sea_orm::DbBackend::Postgres,
                "SELECT COUNT(*) AS count FROM schema_version",
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.try_get::<i64>("", "count").unwrap(), 0);
    }
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-persistence/writer-database", relationship: crate::trace_support::Relationship::Asserts, scope: "実 PostgreSQL と実保存アダプターの公開 API または DB 接続を使用する。", reason: "対応しないスキーマを変更しないこと、または保存時の不整合拒否と読み取り結果を直接検査する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["postgres"], fake: &[] }] };
#[tokio::test]
async fn a_missing_schema_ledger_is_not_recreated_for_existing_game_tables() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    SeaOrmStore::connect(&url)
        .await
        .unwrap()
        .close()
        .await
        .unwrap();
    let raw = Database::connect(&url).await.unwrap();
    raw.execute_unprepared("DROP TABLE schema_version")
        .await
        .unwrap();
    assert_eq!(
        SeaOrmStore::connect(&url).await.map(|_| ()),
        Err(Error::Corrupt)
    );
    let row = raw
        .query_one_raw(sea_orm::Statement::from_string(
            sea_orm::DbBackend::Postgres,
            "SELECT COUNT(*) AS count FROM information_schema.tables WHERE table_schema = current_schema() AND table_name = 'schema_version'",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "count").unwrap(), 0);
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-persistence/writer-database", relationship: crate::trace_support::Relationship::Asserts, scope: "実 PostgreSQL と実保存アダプターの公開 API または DB 接続を使用する。", reason: "対応しないスキーマを変更しないこと、または保存時の不整合拒否と読み取り結果を直接検査する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["postgres"], fake: &[] }] };
#[tokio::test]
async fn fresh_game_database_has_no_account_tables() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let (_store, _service) = setup(&url).await;
    let raw = Database::connect(&url).await.unwrap();
    assert_eq!(
        integer_query(
            &raw,
            "SELECT COUNT(*) AS value FROM information_schema.tables WHERE table_schema = current_schema() AND table_name = 'users'"
        )
        .await,
        0
    );
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-persistence/writer-database", relationship: crate::trace_support::Relationship::Asserts, scope: "実 PostgreSQL と実保存アダプターの公開 API または DB 接続を使用する。", reason: "対応しないスキーマを変更しないこと、または保存時の不整合拒否と読み取り結果を直接検査する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["postgres"], fake: &[] }] };
#[tokio::test]
async fn failed_bootstrap_rolls_back_the_entire_schema() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let raw = Database::connect(&url).await.unwrap();
    raw.execute_unprepared("CREATE VIEW records_finished AS SELECT 1")
        .await
        .unwrap();
    assert_eq!(
        SeaOrmStore::connect(&url).await.map(|_| ()),
        Err(Error::Unavailable)
    );
    assert_eq!(
        integer_query(&raw, "SELECT count(*) AS value FROM information_schema.tables WHERE table_schema = current_schema() AND table_type = 'BASE TABLE'").await,
        0
    );
    raw.execute_unprepared("DROP VIEW records_finished")
        .await
        .unwrap();
    SeaOrmStore::connect(&url)
        .await
        .unwrap()
        .close()
        .await
        .unwrap();
    assert_eq!(
        integer_query(&raw, "SELECT version AS value FROM schema_version").await,
        8
    );
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-persistence/writer-database", relationship: crate::trace_support::Relationship::Asserts, scope: "実 PostgreSQL と実保存アダプターの公開 API または DB 接続を使用する。", reason: "対応しないスキーマを変更しないこと、または保存時の不整合拒否と読み取り結果を直接検査する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["postgres"], fake: &[] }] };
#[tokio::test]
async fn incompatible_or_incomplete_existing_schema_is_not_modified() {
    for sql in [
        "UPDATE schema_version SET version = 0",
        "UPDATE schema_version SET version = 4",
        "UPDATE schema_version SET id = 1",
        "INSERT INTO schema_version VALUES (1, 5)",
        "ALTER TABLE schema_version RENAME COLUMN version TO missing",
        "DROP TABLE schema_version; CREATE TABLE schema_version (id INTEGER, version TEXT); INSERT INTO schema_version VALUES (0, 'invalid')",
        "DROP TABLE schema_version; CREATE TABLE schema_version (id TEXT, version INTEGER); INSERT INTO schema_version VALUES ('invalid', 5)",
        "DROP TABLE outbox_events",
    ] {
        let database = TestDatabase::new().await;
        let url = database.url().to_string();
        let (_, service) = setup(&url).await;
        service
            .make_move(&id(), movement("m", 0, "black", "7g7f"))
            .await
            .unwrap();
        let raw = Database::connect(&url).await.unwrap();
        raw.execute_unprepared(sql).await.unwrap();
        assert_eq!(
            SeaOrmStore::connect(&url).await.map(|_| ()),
            Err(Error::Corrupt),
            "{sql}"
        );
        assert_eq!(
            integer_query(&raw, "SELECT count(*) AS value FROM games").await,
            1
        );
        assert_eq!(
            integer_query(&raw, "SELECT count(*) AS value FROM journal").await,
            1
        );
    }
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-persistence/writer-database", relationship: crate::trace_support::Relationship::Asserts, scope: "実 PostgreSQL と実保存アダプターの公開 API または DB 接続を使用する。", reason: "対応しないスキーマを変更しないこと、または保存時の不整合拒否と読み取り結果を直接検査する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["postgres"], fake: &[] }] };
#[tokio::test]
async fn unrelated_tables_require_a_current_application_schema() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let raw = Database::connect(&url).await.unwrap();
    raw.execute_unprepared(
        "CREATE TABLE application_extra (value TEXT); INSERT INTO application_extra VALUES ('preserved')",
    )
    .await
    .unwrap();
    assert_eq!(
        SeaOrmStore::connect(&url).await.map(|_| ()),
        Err(Error::Corrupt)
    );
    assert_eq!(
        integer_query(
            &raw,
            "SELECT count(*) AS value FROM information_schema.tables WHERE table_schema = current_schema() AND table_type = 'BASE TABLE'"
        )
        .await,
        1
    );
    raw.execute_unprepared("DROP TABLE application_extra")
        .await
        .unwrap();
    SeaOrmStore::connect(&url)
        .await
        .unwrap()
        .close()
        .await
        .unwrap();
    raw.execute_unprepared(
        "CREATE TABLE application_extra (value TEXT); INSERT INTO application_extra VALUES ('preserved')",
    )
    .await
    .unwrap();
    SeaOrmStore::connect(&url)
        .await
        .unwrap()
        .close()
        .await
        .unwrap();
    assert_eq!(
        integer_query(
            &raw,
            "SELECT count(*) AS value FROM application_extra WHERE value = 'preserved'"
        )
        .await,
        1
    );
}
}
