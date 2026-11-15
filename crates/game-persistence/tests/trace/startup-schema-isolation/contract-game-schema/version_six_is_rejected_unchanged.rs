crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL version-six fixture rejection with unchanged schema/data snapshot.",
        reason: "A version-six fixture is rejected and its before/after schema/data snapshot remains equal.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn version_six_is_rejected_unchanged() {
    let database = TestDatabase::new().await;
    let raw = Database::connect(database.url()).await.unwrap();
    raw.execute_unprepared(include_str!("../../../fixtures/schema-v6.sql"))
        .await
        .unwrap();
    let before = snapshot(&raw).await;
    assert_eq!(
        SeaOrmStore::connect(database.url()).await.map(|_| ()),
        Err(Error::Corrupt)
    );
    assert_eq!(snapshot(&raw).await, before);
}
}
