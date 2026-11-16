use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実 PostgreSQL で、選択したスキーマ・台帳の変更と、再接続の拒否を確認する。",
        reason: "台帳の空状態・欠落、索引の欠落、NULL を許すフィールド、外部キー、バージョン制約に対する選択した変更のいずれでも、再接続が Corrupt となることを確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
    async fn missing_ledger_or_changed_indexes_fail_closed() {
        for mutation in [
            "DELETE FROM account_schema_version",
            "DROP TABLE account_schema_version",
            "DROP INDEX sessions_expiry",
            "ALTER TABLE accounts ALTER COLUMN display_name DROP NOT NULL",
            "ALTER TABLE external_identities DROP CONSTRAINT external_identities_user_id_fkey",
            "ALTER TABLE account_schema_version DROP CONSTRAINT account_schema_version_version_check",
        ] {
            let database = pg_test_support::TestDatabase::new().await;
            let store = PostgresAccounts::connect(database.url()).await.unwrap();
            store.db.execute_unprepared(mutation).await.unwrap();
            store.close().await.unwrap();
            assert!(
                matches!(
                    PostgresAccounts::connect(database.url()).await,
                    Err(Error::Corrupt)
                ),
                "{mutation}"
            );
        }
    }
}

#[tokio::test]
async fn schema_constraints_and_ledger_are_checked_on_reopen() {
    let database = pg_test_support::TestDatabase::new().await;
    let url = database.url();
    let store = PostgresAccounts::connect(url).await.unwrap();
    store
        .db
        .execute_unprepared("DROP TABLE external_identities")
        .await
        .unwrap();
    store
        .db
        .execute_unprepared(
            "CREATE TABLE external_identities (issuer TEXT, subject TEXT, user_id TEXT)",
        )
        .await
        .unwrap();
    store.close().await.unwrap();
    assert!(matches!(
        PostgresAccounts::connect(url).await,
        Err(Error::Corrupt)
    ));
}
