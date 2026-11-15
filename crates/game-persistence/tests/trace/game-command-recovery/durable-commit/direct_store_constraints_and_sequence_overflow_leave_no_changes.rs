#[tokio::test]
async fn direct_store_constraints_and_sequence_overflow_leave_no_changes() {
    let database = TestDatabase::new().await;
    let (store, service) = setup(database.url()).await;
    let receipt = Receipt {
        request: MutationRequest {
            request_id: RequestId::new("r").unwrap(),
            user_id: user("outsider"),
            expected_seq: 0,
            action: Action::Move("7g7f".parse().unwrap()),
        },
        acceptance: Acceptance {
            seq: 1,
            in_check: false,
            result: None,
        },
    };
    assert_eq!(
        store
            .commit_change(&StagedGameChange::new(id(), receipt.clone()).unwrap())
            .await,
        Err(Error::NonParticipant)
    );
    assert_eq!(store.version(&id()).await.unwrap(), 0);
    let mut bad = receipt.clone();
    bad.request.expected_seq = u64::MAX;
    bad.acceptance.seq = 0;
    assert_eq!(
        StagedGameChange::new(id(), bad),
        Err(Error::SequenceExhausted)
    );
    assert_eq!(store.version(&id()).await.unwrap(), 0);
    assert!(store.load_game(&id()).await.unwrap().journal.is_empty());
    service
        .make_move(&id(), movement("r", 0, "black", "7g7f"))
        .await
        .unwrap();
    let mut duplicate = receipt;
    duplicate.request.user_id = user("white");
    duplicate.request.expected_seq = 1;
    duplicate.acceptance.seq = 2;
    duplicate.request.action = Action::Move("3c3d".parse().unwrap());
    assert_eq!(
        store
            .commit_change(&StagedGameChange::new(id(), duplicate).unwrap())
            .await,
        Err(Error::Unavailable)
    );
    assert_eq!(store.version(&id()).await.unwrap(), 1);
    assert_eq!(store.load_game(&id()).await.unwrap().journal.len(), 1);
}
