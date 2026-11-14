use async_trait::async_trait;
use game_application::{
    Acceptance, Action, ApplicationError as Error, GameId, GameRecord, GameRepository, GameService,
    MakeMove, MutationRequest, Participants, Receipt, RequestId, Resign, StagedGameChange, UserId,
};
use game_persistence::SeaOrmStore;
use pg_test_support::TestDatabase;
use sea_orm::{ConnectionTrait, Database};
use shogi_domain::Player;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

fn user(value: &str) -> UserId {
    UserId::new(value).unwrap()
}
fn id() -> GameId {
    GameId::new("game").unwrap()
}
fn participants() -> Participants {
    Participants::new(user("black"), user("white")).unwrap()
}
fn movement(request: &str, expected_seq: u64, user_id: &str, usi: &str) -> MakeMove {
    MakeMove {
        request_id: RequestId::new(request).unwrap(),
        expected_seq,
        user_id: user(user_id),
        movement: usi.parse().unwrap(),
    }
}
fn resign() -> Resign {
    Resign {
        request_id: RequestId::new("end").unwrap(),
        expected_seq: 5,
        user_id: user("black"),
    }
}
async fn setup(url: &str) -> (Arc<SeaOrmStore>, GameService) {
    let store = Arc::new(SeaOrmStore::connect(url).await.unwrap());
    let service = GameService::new(store.clone());
    service.create(id(), participants()).await.unwrap();
    (store, service)
}

#[tokio::test]
async fn closing_the_pool_makes_the_game_service_unavailable() {
    let database = TestDatabase::new().await;
    let (store, games) = setup(database.url()).await;
    assert_eq!(games.snapshot(&id()).await.unwrap().seq, 0);

    store.close().await.unwrap();

    assert_eq!(games.snapshot(&id()).await, Err(Error::Unavailable));
}

#[tokio::test]
async fn distinct_ids_create_games_without_accounts_and_bindings_are_immutable() {
    {
        let database = TestDatabase::new().await;
        let url = database.url().to_string();
        let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
        let service = GameService::new(store);
        assert_eq!(
            Participants::new(user("black"), user("black")),
            Err(Error::InvalidParticipants)
        );
        let before = service.create(id(), participants()).await.unwrap();
        assert_eq!(
            service
                .create(
                    id(),
                    Participants::new(user("white"), user("black")).unwrap()
                )
                .await,
            Err(Error::AlreadyExists)
        );
        assert_eq!(service.snapshot(&id()).await.unwrap(), before);
        let accepted = service
            .make_move(&id(), movement("first", 0, "black", "7g7f"))
            .await
            .unwrap();
        assert_eq!(accepted.seq, 1);
        assert_eq!(
            service
                .make_move(&id(), movement("first", 0, "outsider", "7g7f"))
                .await,
            Err(Error::NonParticipant)
        );
    }
}

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
    let service = GameService::new(store.clone());
    assert_eq!(service.snapshot(&id()).await.unwrap(), ongoing);
    assert_eq!(service.make_move(&id(), first.clone()).await, Ok(receipt));
    let finish = service.resign(&id(), resign()).await.unwrap();
    assert_eq!(finish.result.unwrap().winner, Player::White);
    let finished = service.snapshot(&id()).await.unwrap();
    drop(service);
    store.close().await.unwrap();
    let service = GameService::new(Arc::new(SeaOrmStore::connect(&url).await.unwrap()));
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn independent_instances_cas_refresh_and_identical_concurrent_replay() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let (store, a) = setup(&url).await;
    let b = GameService::new(Arc::new(SeaOrmStore::connect(&url).await.unwrap()));
    assert_eq!(b.snapshot(&id()).await.unwrap().seq, 0);
    let game = id();
    let (left, right) = tokio::join!(
        a.make_move(&game, movement("a", 0, "black", "7g7f")),
        b.make_move(&game, movement("b", 0, "black", "2g2f"))
    );
    assert!(matches!(
        (left, right),
        (Ok(_), Err(Error::SequenceMismatch { actual: 1, .. }))
            | (Err(Error::SequenceMismatch { actual: 1, .. }), Ok(_))
    ));
    assert_eq!(
        a.snapshot(&game).await.unwrap(),
        b.snapshot(&game).await.unwrap()
    );
    let command = movement("white", 1, "white", "3c3d");
    let (left, right) = tokio::join!(
        a.make_move(&game, command.clone()),
        b.make_move(&game, command)
    );
    assert_eq!(left.unwrap(), right.unwrap());
    assert_eq!(store.load_game(&game).await.unwrap().journal.len(), 2);
    store.close().await.unwrap();
    assert_eq!(a.snapshot(&game).await, Err(Error::Unavailable));
    assert_eq!(
        a.make_move(&game, movement("white", 1, "white", "3c3d"))
            .await,
        Err(Error::Unavailable)
    );
}

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
        assert_eq!(service.snapshot(&id()).await.unwrap(), before);
        raw.execute_unprepared("DROP TRIGGER fail_commit_change ON journal")
            .await
            .unwrap();
        assert_eq!(service.make_move(&id(), command).await.unwrap().seq, 1);
    }
}

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
        let restored = GameService::new(Arc::new(SeaOrmStore::connect(&url).await.unwrap()));
        assert_eq!(restored.snapshot(&id()).await, Err(Error::Corrupt), "{sql}");
    }
}

struct FaultStore {
    inner: Arc<SeaOrmStore>,
    mode: AtomicU8,
    unreadable: AtomicBool,
}
#[async_trait]
impl GameRepository for FaultStore {
    async fn create_rated_game(
        &self,
        id: GameId,
        participants: Participants,
        pool: game_application::rating::Pool,
    ) -> Result<(), Error> {
        self.inner.create_rated_game(id, participants, pool).await
    }
    async fn rated_game(
        &self,
        id: &GameId,
    ) -> Result<Option<game_application::rating::RatedGame>, Error> {
        self.inner.rated_game(id).await
    }

    async fn load_game_for_viewer(
        &self,
        id: &GameId,
        viewer: &UserId,
    ) -> Result<game_application::ViewerGameRecord, Error> {
        self.inner.load_game_for_viewer(id, viewer).await
    }
    async fn view_metadata(
        &self,
        id: &GameId,
        viewer: &UserId,
    ) -> Result<game_application::ViewMetadata, Error> {
        self.inner.view_metadata(id, viewer).await
    }
    async fn create_game(&self, id: GameId, participants: Participants) -> Result<(), Error> {
        self.inner.create_game(id, participants).await
    }
    async fn load_game(&self, id: &GameId) -> Result<GameRecord, Error> {
        if self.unreadable.load(Ordering::SeqCst) {
            Err(Error::Unavailable)
        } else {
            self.inner.load_game(id).await
        }
    }
    async fn version(&self, id: &GameId) -> Result<u64, Error> {
        if self.unreadable.load(Ordering::SeqCst) {
            Err(Error::Unavailable)
        } else {
            self.inner.version(id).await
        }
    }
    async fn commit_change(&self, change: &StagedGameChange) -> Result<(), Error> {
        let mode = self.mode.swap(0, Ordering::SeqCst);
        if mode == 1 {
            return Err(Error::Unavailable);
        }
        if mode == 4 {
            let mut different = change.receipt().clone();
            different.request.user_id = user("white");
            different.request.action = Action::Resign;
            different.acceptance.result = Some(shogi_domain::GameResult {
                winner: Player::Black,
                reason: shogi_domain::FinishReason::Resignation,
            });
            let different = StagedGameChange::new(change.game_id().clone(), different)?;
            self.inner.commit_change(&different).await?;
        } else {
            self.inner.commit_change(change).await?;
        }
        if mode == 3 {
            self.unreadable.store(true, Ordering::SeqCst);
        }
        if mode >= 2 {
            Err(Error::Unavailable)
        } else {
            Ok(())
        }
    }
}

#[tokio::test]
async fn uncertain_outcomes_reconcile_durable_receipts_or_fence_until_recovery() {
    for mode in [1, 2, 3, 4] {
        let database = TestDatabase::new().await;
        let (store, _) = setup(database.url()).await;
        let fault = Arc::new(FaultStore {
            inner: store.clone(),
            mode: AtomicU8::new(mode),
            unreadable: AtomicBool::new(false),
        });
        let service = GameService::new(fault.clone());
        let command = movement("request", 0, "black", "7g7f");
        let response = service.make_move(&id(), command.clone()).await;
        match mode {
            2 => assert_eq!(response.unwrap().seq, 1),
            4 => assert_eq!(response, Err(Error::RequestConflict)),
            _ => assert_eq!(response, Err(Error::Unavailable)),
        }
        if mode == 3 {
            assert_eq!(service.snapshot(&id()).await, Err(Error::Unavailable));
            assert_eq!(
                service
                    .resign(
                        &id(),
                        Resign {
                            request_id: RequestId::new("next").unwrap(),
                            expected_seq: 1,
                            user_id: user("white")
                        }
                    )
                    .await,
                Err(Error::Unavailable)
            );
            fault.unreadable.store(false, Ordering::SeqCst);
        }
        let record = store.load_game(&id()).await.unwrap();
        assert_eq!(record.journal.len(), usize::from(mode != 1));
        assert_eq!(
            service.snapshot(&id()).await.unwrap().seq,
            u64::from(mode != 1)
        );
        if mode != 4 {
            assert_eq!(service.make_move(&id(), command).await.unwrap().seq, 1);
        }
        assert_eq!(store.load_game(&id()).await.unwrap().journal.len(), 1);
    }
}

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

#[tokio::test]
async fn postgres_enforces_result_pairs_foreign_keys_and_distinct_participants() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let (_, service) = setup(&url).await;
    service
        .make_move(&id(), movement("m", 0, "black", "7g7f"))
        .await
        .unwrap();
    let raw = Database::connect(&url).await.unwrap();
    for sql in [
        "UPDATE games SET winner = 1, reason = NULL",
        "UPDATE games SET winner = NULL, reason = 2",
        "UPDATE journal SET winner = 1, reason = NULL",
        "UPDATE journal SET winner = NULL, reason = 2",
        "UPDATE games SET black = white",
        "DELETE FROM games WHERE id = 'game'",
        "UPDATE journal SET game_id = 'absent'",
        "DELETE FROM journal WHERE seq = 1",
    ] {
        assert!(raw.execute_unprepared(sql).await.is_err(), "{sql}");
    }
    assert_eq!(service.snapshot(&id()).await.unwrap().seq, 1);
}

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
        let service = GameService::new(reader.clone());
        service.snapshot(&id()).await.unwrap();
    }
    writing.await.unwrap();
}

async fn integer_query(db: &sea_orm::DatabaseConnection, sql: &str) -> i64 {
    db.query_one_raw(sea_orm::Statement::from_string(
        sea_orm::DbBackend::Postgres,
        sql,
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "value")
    .unwrap()
}

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
        7
    );
}

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

#[tokio::test]
async fn identifiers_with_nul_and_backslashes_round_trip_without_collisions() {
    let database = TestDatabase::new().await;
    let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
    let games = GameService::new(store.clone());
    let black = user("black\0\\name");
    let white = user("white\\0name");
    let game = GameId::new("game\0\\name").unwrap();
    let literal = GameId::new("game\\0\\name").unwrap();
    for id in [&game, &literal] {
        games
            .create(
                id.clone(),
                Participants::new(black.clone(), white.clone()).unwrap(),
            )
            .await
            .unwrap();
    }
    let command = MakeMove {
        request_id: RequestId::new("request\0\\name").unwrap(),
        expected_seq: 0,
        user_id: black.clone(),
        movement: "7g7f".parse().unwrap(),
    };
    let accepted = games.make_move(&game, command.clone()).await.unwrap();
    store.close().await.unwrap();
    let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
    let restored = GameService::new(store.clone());
    assert_eq!(
        restored.make_move(&game, command.clone()).await.unwrap(),
        accepted
    );
    let record = store.load_game(&game).await.unwrap();
    assert_eq!(record.id, game);
    assert_eq!(
        record.participants,
        Participants::new(black, white).unwrap()
    );
    assert_eq!(record.journal[0].request.request_id, command.request_id);
    assert_eq!(restored.snapshot(&literal).await.unwrap().seq, 0);
}
