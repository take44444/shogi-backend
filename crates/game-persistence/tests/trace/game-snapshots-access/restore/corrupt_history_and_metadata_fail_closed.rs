crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport. Direct SQL corrupts seven journal/metadata cases.",
        reason: "Changed check/USI/owner/request, version, terminal metadata and deleted receipt all yield Corrupt on restoration.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn corrupt_history_and_metadata_fail_closed() {
    for sql in [
        "UPDATE journal SET in_check = TRUE WHERE seq = 1",
        "UPDATE journal SET usi = '7g7e' WHERE seq = 1",
        "UPDATE journal SET user_id = 'outsider' WHERE seq = 1",
        "UPDATE journal SET request_id = 'bad id' WHERE seq = 1",
        "UPDATE games SET version = 2",
        "UPDATE games SET winner = 1, reason = 2, finished_at = 1",
        "DELETE FROM journal WHERE seq = 1",
    ] {
        let database = TestDatabase::new().await;
        let url = database.url().to_string();
        let (_, service) = setup(&url).await;
        service
            .make_move(&id(), movement("m", 0, "black", "7g7f"))
            .await
            .unwrap();
        let raw = Database::connect(&url).await.unwrap();
        if sql.starts_with("DELETE FROM journal") {
            raw.execute_unprepared("DELETE FROM outbox_events")
                .await
                .unwrap();
        }
        raw.execute_unprepared(sql).await.unwrap();
        let restored = pg_test_support::GameFixture::new(
            Arc::new(SeaOrmStore::connect(&url).await.unwrap()),
            &url,
        );
        assert_eq!(restored.snapshot(&id()).await, Err(Error::Corrupt), "{sql}");
    }
}
}
