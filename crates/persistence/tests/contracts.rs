use async_trait::async_trait;
use game_application::{
    Acceptance, Action, ApplicationError as Error, GameId, GameRecord, GameRepository, GameService,
    MakeMove, Participants, Receipt, RequestId, Resign, StagedGameChange, UserId, UserService,
};
use persistence::SeaOrmStore;
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
    Participants {
        black: user("black"),
        white: user("white"),
    }
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
fn file_url(dir: &tempfile::TempDir) -> String {
    format!("sqlite://{}?mode=rwc", dir.path().join("game.db").display())
}
async fn setup(url: &str) -> (Arc<SeaOrmStore>, GameService) {
    let store = Arc::new(SeaOrmStore::connect(url).await.unwrap());
    let users = UserService::new(store.clone());
    let service = GameService::new(store.clone());
    for name in ["black", "white", "outsider"] {
        users.create_user(user(name)).await.unwrap();
    }
    service.create(id(), participants()).await.unwrap();
    (store, service)
}

#[tokio::test]
async fn user_and_game_services_share_one_pool_including_shutdown() {
    let (store, games) = setup("sqlite::memory:").await;
    let users = UserService::new(store.clone());
    assert_eq!(
        users.get_user(&user("black")).await.unwrap().id,
        user("black")
    );
    assert_eq!(games.snapshot(&id()).await.unwrap().seq, 0);

    store.close().await.unwrap();

    assert_eq!(
        users.get_user(&user("black")).await,
        Err(Error::Unavailable)
    );
    assert_eq!(games.snapshot(&id()).await, Err(Error::Unavailable));
}

#[tokio::test]
async fn user_constraints_and_immutable_participants() {
    for url in ["sqlite::memory:", &file_url(&tempfile::tempdir().unwrap())] {
        let store = Arc::new(SeaOrmStore::connect(url).await.unwrap());
        let users = UserService::new(store.clone());
        let service = GameService::new(store);
        assert_eq!(
            users.get_user(&user("black")).await,
            Err(Error::UserNotFound)
        );
        assert_eq!(
            users.create_user(user("black")).await.unwrap().id,
            user("black")
        );
        assert_eq!(
            users.create_user(user("black")).await,
            Err(Error::AlreadyExists)
        );
        assert_eq!(
            service.create(id(), participants()).await,
            Err(Error::UserNotFound)
        );
        users.create_user(user("white")).await.unwrap();
        assert_eq!(
            users.get_user(&user("white")).await.unwrap().id,
            user("white")
        );
        assert_eq!(
            service
                .create(
                    id(),
                    Participants {
                        black: user("black"),
                        white: user("black")
                    }
                )
                .await,
            Err(Error::InvalidParticipants)
        );
        let before = service.create(id(), participants()).await.unwrap();
        assert_eq!(
            service
                .create(
                    id(),
                    Participants {
                        black: user("white"),
                        white: user("black")
                    }
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
async fn file_restart_restores_position_hands_promotion_and_receipts_even_after_finish() {
    let dir = tempfile::tempdir().unwrap();
    let url = file_url(&dir);
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
    let users = UserService::new(store.clone());
    let service = GameService::new(store.clone());
    assert_eq!(service.snapshot(&id()).await.unwrap(), ongoing);
    assert_eq!(service.make_move(&id(), first.clone()).await, Ok(receipt));
    assert_eq!(
        users.get_user(&user("black")).await.unwrap().id,
        user("black")
    );
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
    let dir = tempfile::tempdir().unwrap();
    let url = file_url(&dir);
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
        let dir = tempfile::tempdir().unwrap();
        let url = file_url(&dir);
        let (store, service) = setup(&url).await;
        let raw = Database::connect(&url).await.unwrap();
        if commit_failure {
            raw.execute_unprepared("CREATE TABLE deferred_failure (user_id TEXT REFERENCES users(id) DEFERRABLE INITIALLY DEFERRED)").await.unwrap();
            raw.execute_unprepared("CREATE TRIGGER fail_commit_change AFTER INSERT ON journal BEGIN INSERT INTO deferred_failure VALUES ('absent'); END").await.unwrap();
        } else {
            raw.execute_unprepared("CREATE TRIGGER fail_commit_change BEFORE INSERT ON journal BEGIN SELECT RAISE(ABORT, 'injected write failure'); END").await.unwrap();
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
        raw.execute_unprepared("DROP TRIGGER fail_commit_change")
            .await
            .unwrap();
        assert_eq!(service.make_move(&id(), command).await.unwrap().seq, 1);
    }
}

#[tokio::test]
async fn direct_store_constraints_and_sequence_overflow_leave_no_changes() {
    let (store, service) = setup("sqlite::memory:").await;
    let receipt = Receipt {
        request_id: RequestId::new("r").unwrap(),
        user_id: user("outsider"),
        expected_seq: 0,
        action: Action::Move("7g7f".parse().unwrap()),
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
    bad.expected_seq = u64::MAX;
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
    duplicate.user_id = user("white");
    duplicate.expected_seq = 1;
    duplicate.acceptance.seq = 2;
    duplicate.action = Action::Move("3c3d".parse().unwrap());
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
        "UPDATE journal SET in_check = 1 WHERE seq = 1",
        "UPDATE journal SET usi = '7g7e' WHERE seq = 1",
        "UPDATE journal SET user_id = 'outsider' WHERE seq = 1",
        "UPDATE journal SET request_id = 'bad id' WHERE seq = 1",
        "UPDATE games SET version = 2",
        "UPDATE games SET winner = 1, reason = 2",
        "DELETE FROM journal WHERE seq = 1",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let url = file_url(&dir);
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
            different.user_id = user("white");
            different.action = Action::Resign;
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
        let (store, _) = setup("sqlite::memory:").await;
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
    let dir = tempfile::tempdir().unwrap();
    let url = file_url(&dir);
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..6 {
        let url = url.clone();
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
        let dir = tempfile::tempdir().unwrap();
        let url = file_url(&dir);
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
                sea_orm::DbBackend::Sqlite,
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
    let dir = tempfile::tempdir().unwrap();
    let url = file_url(&dir);
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
            sea_orm::DbBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE name = 'schema_version'",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "count").unwrap(), 0);
}

#[tokio::test]
async fn sqlite_enforces_result_pairs_foreign_keys_and_distinct_participants() {
    let dir = tempfile::tempdir().unwrap();
    let url = file_url(&dir);
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
        "UPDATE games SET black = 'absent'",
        "UPDATE journal SET user_id = 'absent'",
        "DELETE FROM users WHERE id = 'black'",
    ] {
        assert!(raw.execute_unprepared(sql).await.is_err(), "{sql}");
    }
    assert_eq!(service.snapshot(&id()).await.unwrap().seq, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn joined_history_load_remains_coherent_during_concurrent_commits() {
    let dir = tempfile::tempdir().unwrap();
    let url = file_url(&dir);
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
