use super::*;

#[test]
fn ids_validate_bytes_and_unicode_whitespace() {
    for invalid in [
        "".to_owned(),
        " ".into(),
        "x\ty".into(),
        "x\ny".into(),
        "x\u{2003}y".into(),
        "x".repeat(129),
        "é".repeat(65),
    ] {
        assert!(GameId::new(invalid.clone()).is_err());
        assert!(RequestId::new(invalid).is_err());
    }
    for valid in ["game-1".to_owned(), "x".repeat(128), "é".repeat(64)] {
        assert_eq!(GameId::new(valid.clone()).unwrap().as_str(), valid);
        assert_eq!(RequestId::new(valid.clone()).unwrap().as_str(), valid);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_creation_has_exactly_one_winner_and_shared_state() {
    let (_database, service) = setup().await;
    let barrier = Arc::new(Barrier::new(24));
    let mut tasks = JoinSet::new();
    for _ in 0..24 {
        let service = service.clone();
        let barrier = barrier.clone();
        tasks.spawn(async move {
            barrier.wait().await;
            service.seed(id("same"), participants()).await
        });
    }
    let mut created = 0;
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(snapshot) => {
                created += 1;
                assert_eq!(snapshot.seq, 0);
            }
            Err(error) => assert_eq!(error, Error::AlreadyExists),
        }
    }
    assert_eq!(created, 1);
    service
        .clone()
        .make_move(&id("same"), movement("m", 0, Player::Black, "7g7f"))
        .await
        .unwrap();
    assert_eq!(service.snapshot(&id("same")).await.unwrap().seq, 1);
}
