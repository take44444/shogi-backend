use async_trait::async_trait;
use game_application::{
    ApplicationError as Error, DomainEvent, GameId, GameRecord, GameRepository, GameService,
    MakeMove, OutboxRepository, Participants, RequestId, Resign, StagedGameChange, UserId,
};
use game_persistence::{SeaOrmStore, SqliteOutbox};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use std::sync::Arc;

fn id() -> GameId {
    GameId::new("game").unwrap()
}
fn user(s: &str) -> UserId {
    UserId::new(s).unwrap()
}
fn command() -> MakeMove {
    MakeMove {
        request_id: RequestId::new("move").unwrap(),
        expected_seq: 0,
        user_id: user("black"),
        movement: "7g7f".parse().unwrap(),
    }
}
fn url(dir: &tempfile::TempDir) -> String {
    format!("sqlite://{}?mode=rwc", dir.path().join("game.db").display())
}
async fn setup(url: &str) -> (Arc<SeaOrmStore>, GameService, DatabaseConnection) {
    let store = Arc::new(SeaOrmStore::connect(url).await.unwrap());
    let service = GameService::new(store.clone());
    service
        .create(
            id(),
            Participants {
                black: user("black"),
                white: user("white"),
            },
        )
        .await
        .unwrap();
    (store, service, Database::connect(url).await.unwrap())
}
async fn count(db: &DatabaseConnection, table: &str) -> i64 {
    db.query_one_raw(Statement::from_string(
        DbBackend::Sqlite,
        format!("SELECT COUNT(*) AS count FROM {table}"),
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "count")
    .unwrap()
}

#[tokio::test]
async fn retry_rejection_restoration_and_frozen_payload() {
    let dir = tempfile::tempdir().unwrap();
    let url = url(&dir);
    let (store, service, raw) = setup(&url).await;
    assert_eq!(count(&raw, "outbox_events").await, 0);
    assert!(
        service
            .make_move(
                &id(),
                MakeMove {
                    user_id: user("white"),
                    ..command()
                }
            )
            .await
            .is_err()
    );
    assert_eq!(count(&raw, "outbox_events").await, 0);
    let accepted = service.make_move(&id(), command()).await.unwrap();
    assert_eq!(service.make_move(&id(), command()).await.unwrap(), accepted);
    let restored = GameService::new(store.clone());
    assert_eq!(restored.snapshot(&id()).await.unwrap().seq, 1);
    assert_eq!(
        restored.make_move(&id(), command()).await.unwrap(),
        accepted
    );
    assert_eq!(count(&raw, "outbox_events").await, 1);
    service
        .resign(
            &id(),
            Resign {
                request_id: RequestId::new("end").unwrap(),
                expected_seq: 1,
                user_id: user("white"),
            },
        )
        .await
        .unwrap();
    assert_eq!(count(&raw, "outbox_events").await, 2);
    raw.execute_unprepared("UPDATE journal SET usi = '2g2f' WHERE seq = 1")
        .await
        .unwrap();
    let mut queue = SqliteOutbox::connect(&url).await.unwrap();
    let item = queue.next_pending().await.unwrap().unwrap();
    assert!(
        matches!(item.event.event, DomainEvent::MoveCommitted { movement, .. } if movement.to_string() == "7g7f")
    );
    queue.mark_published(&item).await.unwrap();
    queue.mark_published(&item).await.unwrap();
    let end = queue.next_pending().await.unwrap().unwrap();
    assert_eq!(end.event.seq, 2);
    assert!(matches!(end.event.event, DomainEvent::GameFinished { .. }));
    queue.mark_published(&end).await.unwrap();
    assert!(queue.next_pending().await.unwrap().is_none());
}
#[tokio::test]
async fn failed_enqueue_rolls_back_metadata_journal_and_events() {
    let dir = tempfile::tempdir().unwrap();
    let url = url(&dir);
    let (store, service, raw) = setup(&url).await;
    raw.execute_unprepared("CREATE TRIGGER fail_outbox BEFORE INSERT ON outbox_events BEGIN SELECT RAISE(ABORT, 'injected outbox failure'); END").await.unwrap();
    assert_eq!(
        service.make_move(&id(), command()).await,
        Err(Error::Unavailable)
    );
    assert_eq!(store.version(&id()).await.unwrap(), 0);
    assert_eq!(count(&raw, "journal").await, 0);
    assert_eq!(count(&raw, "outbox_events").await, 0);
    raw.execute_unprepared("DROP TRIGGER fail_outbox")
        .await
        .unwrap();
    assert_eq!(service.make_move(&id(), command()).await.unwrap().seq, 1);
    assert_eq!(count(&raw, "outbox_events").await, 1);
}
#[tokio::test]
async fn v1_migration_preserves_history_without_backfill_and_rejects_incomplete_schemas() {
    let dir = tempfile::tempdir().unwrap();
    let url = url(&dir);
    let raw = Database::connect(&url).await.unwrap();
    raw.execute_unprepared(include_str!("fixtures/game_v1.sql"))
        .await
        .unwrap();
    raw.execute_unprepared("INSERT INTO users VALUES ('black'), ('white')")
        .await
        .unwrap();
    raw.execute_unprepared("INSERT INTO games VALUES ('game', 'black', 'white', 1, NULL, NULL)")
        .await
        .unwrap();
    raw.execute_unprepared(
        "INSERT INTO journal VALUES ('game', 1, 'move', 'black', 0, 1, '7g7f', 0, NULL, NULL)",
    )
    .await
    .unwrap();
    let before = GameRecord {
        id: id(),
        participants: Participants {
            black: user("black"),
            white: user("white"),
        },
        version: 1,
        result: None,
        journal: vec![game_application::Receipt {
            request_id: RequestId::new("move").unwrap(),
            user_id: user("black"),
            expected_seq: 0,
            action: game_application::Action::Move("7g7f".parse().unwrap()),
            acceptance: game_application::Acceptance {
                seq: 1,
                in_check: false,
                result: None,
            },
        }],
    };
    let migrated = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
    assert_eq!(migrated.load_game(&id()).await.unwrap(), before);
    let service = GameService::new(migrated);
    assert_eq!(count(&raw, "outbox_events").await, 0);
    service
        .resign(
            &id(),
            Resign {
                request_id: RequestId::new("end").unwrap(),
                expected_seq: 1,
                user_id: user("white"),
            },
        )
        .await
        .unwrap();
    assert_eq!(count(&raw, "outbox_events").await, 1);
    raw.execute_unprepared("UPDATE schema_version SET version = 1")
        .await
        .unwrap();
    assert!(matches!(
        SeaOrmStore::connect(&url).await,
        Err(Error::Corrupt)
    ));
    raw.execute_unprepared("UPDATE schema_version SET version = 2")
        .await
        .unwrap();
    raw.execute_unprepared("DROP TABLE outbox_events")
        .await
        .unwrap();
    assert!(matches!(
        SeaOrmStore::connect(&url).await,
        Err(Error::Corrupt)
    ));
}
#[tokio::test]
async fn unsupported_frozen_encoding_fails_closed() {
    let dir = tempfile::tempdir().unwrap();
    let url = url(&dir);
    let (_, service, raw) = setup(&url).await;
    service.make_move(&id(), command()).await.unwrap();
    raw.execute_unprepared("UPDATE outbox_events SET encoding_version = 99")
        .await
        .unwrap();
    let mut queue = SqliteOutbox::connect(&url).await.unwrap();
    assert!(matches!(queue.next_pending().await, Err(Error::Corrupt)));
}
struct Uncertain(Arc<SeaOrmStore>);
#[async_trait]
impl GameRepository for Uncertain {
    async fn create_game(&self, id: GameId, p: Participants) -> Result<(), Error> {
        self.0.create_game(id, p).await
    }
    async fn load_game(&self, id: &GameId) -> Result<GameRecord, Error> {
        self.0.load_game(id).await
    }
    async fn version(&self, id: &GameId) -> Result<u64, Error> {
        self.0.version(id).await
    }
    async fn commit_change(&self, change: &StagedGameChange) -> Result<(), Error> {
        self.0.commit_change(change).await?;
        Err(Error::Unavailable)
    }
}
#[tokio::test]
async fn uncertain_commit_and_retry_enqueue_once() {
    let dir = tempfile::tempdir().unwrap();
    let url = url(&dir);
    let (store, _, raw) = setup(&url).await;
    let service = GameService::new(Arc::new(Uncertain(store)));
    let accepted = service.make_move(&id(), command()).await.unwrap();
    assert_eq!(service.make_move(&id(), command()).await.unwrap(), accepted);
    assert_eq!(count(&raw, "outbox_events").await, 1);
}
#[test]
fn publisher_lock_probe() {
    let Ok(url) = std::env::var("OUTBOX_LOCK_PROBE_URL") else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let queue = runtime.block_on(SqliteOutbox::connect(&url));
    let expected = std::env::var("OUTBOX_LOCK_PROBE_HELD").unwrap() == "1";
    assert_eq!(queue.is_err(), expected);
    if expected {
        assert!(matches!(
            queue,
            Err(game_persistence::OutboxOpenError::Lock(_))
        ));
    }
}
#[tokio::test]
async fn database_inode_lock_excludes_a_second_process_and_releases_on_exit() {
    let dir = tempfile::tempdir().unwrap();
    let url = url(&dir);
    let queue = SqliteOutbox::connect(&url).await.unwrap();
    let alias = dir.path().join("alias.db");
    std::os::unix::fs::symlink(dir.path().join("game.db"), &alias).unwrap();
    let alias = format!("sqlite://{}?mode=rwc", alias.display());
    for probe_url in [&url, &alias] {
        assert!(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "publisher_lock_probe", "--nocapture"])
                .env("OUTBOX_LOCK_PROBE_URL", probe_url)
                .env("OUTBOX_LOCK_PROBE_HELD", "1")
                .status()
                .unwrap()
                .success()
        );
    }
    drop(queue);
    assert!(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "publisher_lock_probe", "--nocapture"])
            .env("OUTBOX_LOCK_PROBE_URL", &url)
            .env("OUTBOX_LOCK_PROBE_HELD", "0")
            .status()
            .unwrap()
            .success()
    );
    assert!(matches!(
        SqliteOutbox::connect("sqlite::memory:").await,
        Err(game_persistence::OutboxOpenError::FileRequired)
    ));
}

#[tokio::test]
async fn malformed_ledgers_and_orphaned_outbox_tables_fail_closed() {
    for ddl in [
        "CREATE TABLE schema_version (id INTEGER); INSERT INTO schema_version VALUES (0)",
        "CREATE TABLE schema_version (id INTEGER, version TEXT); INSERT INTO schema_version VALUES (0, 'invalid')",
        "CREATE TABLE outbox_events (id INTEGER)",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let url = url(&dir);
        let raw = Database::connect(&url).await.unwrap();
        raw.execute_unprepared(ddl).await.unwrap();
        assert!(
            matches!(SeaOrmStore::connect(&url).await, Err(Error::Corrupt)),
            "{ddl}"
        );
    }
}
