use super::*;

#[traceability::targets(game_application::GameService::make_move)]
#[tokio::test]
async fn real_statement_and_commit_failures_roll_back_both_metadata_and_receipt() {
    for commit_failure in [false, true] {
        let database = TestDatabase::new().await;
        let url = database.url().to_string();
        let (store, service) = setup(&url).await;
        let raw = Database::connect(&url).await.unwrap();
        if commit_failure {
            raw.execute_unprepared("CREATE TABLE deferred_failure (game_id TEXT REFERENCES games(id) DEFERRABLE INITIALLY DEFERRED)").await.unwrap();
            raw.execute_unprepared("CREATE FUNCTION fail_commit_change() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN INSERT INTO deferred_failure VALUES ('absent'); RETURN NEW; END $$; CREATE TRIGGER fail_commit_change AFTER INSERT ON journal FOR EACH ROW EXECUTE FUNCTION fail_commit_change()").await.unwrap();
        } else {
            raw.execute_unprepared("CREATE FUNCTION fail_commit_change() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected write failure'; END $$; CREATE TRIGGER fail_commit_change BEFORE INSERT ON journal FOR EACH ROW EXECUTE FUNCTION fail_commit_change()").await.unwrap();
        }
        let before = service.snapshot(&id()).await.unwrap();
        let command = movement("retry", 0, "black", "7g7f");
        assert_eq!(
            service.make_move(&id(), command.clone()).await,
            Err(Error::Unavailable)
        );
        assert_eq!(store.version(&id()).await.unwrap(), 0);
        assert!(store.load_game(&id()).await.unwrap().journal.is_empty());
        assert_same_snapshot(service.snapshot(&id()).await.unwrap(), &before);
        raw.execute_unprepared("DROP TRIGGER fail_commit_change ON journal")
            .await
            .unwrap();
        assert_eq!(service.make_move(&id(), command).await.unwrap().seq, 1);
    }
}

#[traceability::targets(
    game_persistence::SeaOrmStore::begin_game_update,
    game_application::StagedGameChange::new
)]
#[tokio::test]
async fn direct_store_constraints_and_sequence_overflow_leave_no_changes() {
    let database = TestDatabase::new().await;
    let (store, service) = setup(database.url()).await;
    let receipt = Receipt {
        evaluated_at: game_application::Timestamp::new(0).unwrap(),
        request: MutationRequest {
            request_id: RequestId::new("r").unwrap(),
            user_id: user("outsider"),
            expected_seq: 0,
            action: Action::Move("7g7f".parse().unwrap()),
        },
        acceptance: Acceptance {
            clock: game_application::ClockState::start(
                game_application::TimeControl::for_pool(
                    game_application::rating::Pool::TenMinute30,
                ),
                game_application::Timestamp::new(0).unwrap(),
            )
            .unwrap(),
            seq: 1,
            in_check: false,
            result: None,
        },
    };
    assert_eq!(
        store
            .begin_game_update(&id(), false)
            .await
            .unwrap()
            .commit(&StagedGameChange::new(id(), receipt.clone()).unwrap())
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
            .begin_game_update(&id(), false)
            .await
            .unwrap()
            .commit(&StagedGameChange::new(id(), duplicate).unwrap())
            .await,
        Err(Error::Unavailable)
    );
    assert_eq!(store.version(&id()).await.unwrap(), 1);
    assert_eq!(store.load_game(&id()).await.unwrap().journal.len(), 1);
}
