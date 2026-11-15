crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL version-five schema fixture rejected without migration/deletion.",
        reason: "A version-five fixture is rejected without migration or deletion.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn version_five_is_rejected_without_migration_or_deletion() {
    let database = TestDatabase::new().await;
    let raw = Database::connect(database.url()).await.unwrap();
    raw.execute_unprepared(include_str!("../../../fixtures/schema-v5.sql"))
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
}
