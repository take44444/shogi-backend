crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL with SeaOrmStore and application operations; no browser or RPC transport. Concurrent real commits and ordinary repository loads.",
        reason: "Concurrent writer/read loop compares version with journal length and final result with last receipt; independent snapshot reconstruction succeeds.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn joined_history_load_remains_coherent_during_concurrent_commits() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let (_, writer) = setup(&url).await;
    let reader = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
    let writing = tokio::spawn(async move {
        for seq in 0..24 {
            let state = writer.snapshot(&id()).await.unwrap();
            if state.result.is_some() {
                break;
            }
            writer
                .make_move(
                    &id(),
                    movement(
                        &format!("m-{seq}"),
                        seq,
                        if state.turn == Player::Black {
                            "black"
                        } else {
                            "white"
                        },
                        &state.legal_moves[0].to_string(),
                    ),
                )
                .await
                .unwrap();
        }
    });
    for _ in 0..50 {
        let record = reader.load_game(&id()).await.unwrap();
        assert_eq!(record.version, record.journal.len() as u64);
        assert_eq!(
            record.result,
            record
                .journal
                .last()
                .and_then(|receipt| receipt.acceptance.result)
        );
        let service = pg_test_support::GameFixture::new(reader.clone(), &url);
        service.snapshot(&id()).await.unwrap();
    }
    writing.await.unwrap();
}
}
