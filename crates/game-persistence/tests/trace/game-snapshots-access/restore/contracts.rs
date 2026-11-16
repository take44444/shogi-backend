use super::*;

#[tokio::test]
async fn reconnecting_restores_position_hands_promotion_and_receipts_even_after_finish() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let (store, service) = setup(&url).await;
    let first = movement("m0", 0, "black", "7g7f");
    let receipt = service.make_move(&id(), first.clone()).await.unwrap();
    for (i, (name, usi)) in [
        ("white", "3c3d"),
        ("black", "8h2b+"),
        ("white", "3a2b"),
        ("black", "B*4e"),
    ]
    .into_iter()
    .enumerate()
    {
        service
            .make_move(
                &id(),
                movement(&format!("m{}", i + 1), i as u64 + 1, name, usi),
            )
            .await
            .unwrap();
    }
    let ongoing = service.snapshot(&id()).await.unwrap();
    drop(service);
    store.close().await.unwrap();
    let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
    let service = pg_test_support::GameFixture::new(store.clone(), &url);
    assert_eq!(service.snapshot(&id()).await.unwrap(), ongoing);
    assert_eq!(service.make_move(&id(), first.clone()).await, Ok(receipt));
    let finish = service.resign(&id(), resign()).await.unwrap();
    assert_eq!(finish.result.unwrap().winner, Player::White);
    let finished = service.snapshot(&id()).await.unwrap();
    drop(service);
    store.close().await.unwrap();
    let service = pg_test_support::GameFixture::new(
        Arc::new(SeaOrmStore::connect(&url).await.unwrap()),
        &url,
    );
    assert_eq!(service.snapshot(&id()).await.unwrap(), finished);
    assert_eq!(service.make_move(&id(), first).await, Ok(receipt));
    assert_eq!(service.resign(&id(), resign()).await, Ok(finish));
    assert_eq!(
        service
            .make_move(&id(), movement("m0", 0, "white", "7g7f"))
            .await,
        Err(Error::RequestConflict)
    );
}

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
