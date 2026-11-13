use async_trait::async_trait;
use game_application::{
    ApplicationError as Error, DomainEvent, GameId, GameRecord, GameRepository, GameService,
    MakeMove, OutboxRepository, Participants, RequestId, Resign, StagedGameChange, UserId,
};
use game_persistence::{PostgresOutbox, SeaOrmStore};
use pg_test_support::TestDatabase;
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
async fn setup(url: &str) -> (Arc<SeaOrmStore>, GameService, DatabaseConnection) {
    let store = Arc::new(SeaOrmStore::connect(url).await.unwrap());
    let service = GameService::new(store.clone());
    service
        .create(
            id(),
            Participants::new(user("black"), user("white")).unwrap(),
        )
        .await
        .unwrap();
    (store, service, Database::connect(url).await.unwrap())
}
async fn count(db: &DatabaseConnection, table: &str) -> i64 {
    db.query_one_raw(Statement::from_string(
        DbBackend::Postgres,
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
    let database = TestDatabase::new().await;
    let url = database.url();
    let (store, service, raw) = setup(url).await;
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
    let mut queue = PostgresOutbox::connect(url).await.unwrap();
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
    let database = TestDatabase::new().await;
    let url = database.url();
    let (store, service, raw) = setup(url).await;
    raw.execute_unprepared("CREATE FUNCTION fail_outbox() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected outbox failure'; END $$; CREATE TRIGGER fail_outbox BEFORE INSERT ON outbox_events FOR EACH ROW EXECUTE FUNCTION fail_outbox()").await.unwrap();
    assert_eq!(
        service.make_move(&id(), command()).await,
        Err(Error::Unavailable)
    );
    assert_eq!(store.version(&id()).await.unwrap(), 0);
    assert_eq!(count(&raw, "journal").await, 0);
    assert_eq!(count(&raw, "outbox_events").await, 0);
    raw.execute_unprepared("DROP TRIGGER fail_outbox ON outbox_events")
        .await
        .unwrap();
    assert_eq!(service.make_move(&id(), command()).await.unwrap().seq, 1);
    assert_eq!(count(&raw, "outbox_events").await, 1);
}
#[tokio::test]
async fn unsupported_frozen_encoding_fails_closed() {
    let database = TestDatabase::new().await;
    let url = database.url();
    let (_, service, raw) = setup(url).await;
    service.make_move(&id(), command()).await.unwrap();
    raw.execute_unprepared("UPDATE outbox_events SET encoding_version = 99")
        .await
        .unwrap();
    let mut queue = PostgresOutbox::connect(url).await.unwrap();
    assert!(matches!(queue.next_pending().await, Err(Error::Corrupt)));
}
struct Uncertain(Arc<SeaOrmStore>);
#[async_trait]
impl GameRepository for Uncertain {
    async fn create_rated_game(
        &self,
        id: GameId,
        participants: Participants,
        pool: game_application::rating::Pool,
    ) -> Result<(), Error> {
        self.0.create_rated_game(id, participants, pool).await
    }
    async fn rated_game(
        &self,
        id: &GameId,
    ) -> Result<Option<game_application::rating::RatedGame>, Error> {
        self.0.rated_game(id).await
    }

    async fn load_game_for_viewer(
        &self,
        id: &GameId,
        viewer: &UserId,
    ) -> Result<game_application::ViewerGameRecord, Error> {
        self.0.load_game_for_viewer(id, viewer).await
    }
    async fn view_metadata(
        &self,
        id: &GameId,
        viewer: &UserId,
    ) -> Result<game_application::ViewMetadata, Error> {
        self.0.view_metadata(id, viewer).await
    }
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
    let database = TestDatabase::new().await;
    let url = database.url();
    let (store, _, raw) = setup(url).await;
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
    let queue = runtime.block_on(PostgresOutbox::connect(&url));
    let expected = std::env::var("OUTBOX_LOCK_PROBE_HELD").unwrap() == "1";
    assert_eq!(queue.is_err(), expected);
    if expected {
        assert!(matches!(
            queue,
            Err(game_persistence::OutboxOpenError::PublisherAlreadyRunning)
        ));
    }
}
#[tokio::test]
async fn publisher_session_lock_excludes_a_second_process_and_releases_on_drop() {
    let database = TestDatabase::new().await;
    let url = database.url();
    let queue = PostgresOutbox::connect(url).await.unwrap();
    assert!(matches!(
        PostgresOutbox::connect(url).await,
        Err(game_persistence::OutboxOpenError::PublisherAlreadyRunning)
    ));
    assert!(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "publisher_lock_probe", "--nocapture"])
            .env("OUTBOX_LOCK_PROBE_URL", url)
            .env("OUTBOX_LOCK_PROBE_HELD", "1")
            .status()
            .unwrap()
            .success()
    );
    drop(queue);
    assert!(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "publisher_lock_probe", "--nocapture"])
            .env("OUTBOX_LOCK_PROBE_URL", url)
            .env("OUTBOX_LOCK_PROBE_HELD", "0")
            .status()
            .unwrap()
            .success()
    );
}

#[tokio::test]
async fn publisher_holds_no_transaction_while_awaiting_delivery_and_writes_continue() {
    let database = TestDatabase::new().await;
    let (_, service, raw) = setup(database.url()).await;
    let mut queue = PostgresOutbox::connect(database.url()).await.unwrap();
    service.make_move(&id(), command()).await.unwrap();
    let pending = queue.next_pending().await.unwrap().unwrap();
    let open_transactions: i64 = raw.query_one_raw(Statement::from_string(
        DbBackend::Postgres,
        "SELECT count(*) AS count FROM pg_stat_activity WHERE datname = current_database() AND application_name = 'shogi-outbox-publisher' AND xact_start IS NOT NULL",
    )).await.unwrap().unwrap().try_get("", "count").unwrap();
    assert_eq!(open_transactions, 0);
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
    queue.mark_published(&pending).await.unwrap();
    assert_eq!(queue.next_pending().await.unwrap().unwrap().event.seq, 2);
}

#[tokio::test]
async fn lost_publisher_session_fails_closed_and_pending_work_can_be_reopened() {
    let database = TestDatabase::new().await;
    let (_, service, raw) = setup(database.url()).await;
    service.make_move(&id(), command()).await.unwrap();
    let mut queue = PostgresOutbox::connect(database.url()).await.unwrap();
    let pending = queue.next_pending().await.unwrap().unwrap();
    raw.query_one_raw(Statement::from_string(
        DbBackend::Postgres,
        "SELECT pg_terminate_backend(pid) AS terminated FROM pg_stat_activity WHERE datname = current_database() AND application_name = 'shogi-outbox-publisher'",
    )).await.unwrap().unwrap();
    assert!(matches!(
        queue.next_pending().await,
        Err(Error::Unavailable)
    ));
    assert!(matches!(
        queue.mark_published(&pending).await,
        Err(Error::Unavailable)
    ));
    let mut replacement = PostgresOutbox::connect(database.url()).await.unwrap();
    let restored = replacement.next_pending().await.unwrap().unwrap();
    assert_eq!(restored.event, pending.event);
    replacement.mark_published(&restored).await.unwrap();
    assert!(replacement.next_pending().await.unwrap().is_none());
}

#[tokio::test]
async fn frozen_events_preserve_identifiers_with_nul_and_backslashes() {
    let database = TestDatabase::new().await;
    let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
    let service = GameService::new(store);
    let game = GameId::new("game\0\\name").unwrap();
    let black = user("black\0\\name");
    let request = RequestId::new("request\0\\name").unwrap();
    service
        .create(
            game.clone(),
            Participants::new(black.clone(), user("white")).unwrap(),
        )
        .await
        .unwrap();
    service
        .make_move(
            &game,
            MakeMove {
                request_id: request.clone(),
                expected_seq: 0,
                user_id: black.clone(),
                movement: "7g7f".parse().unwrap(),
            },
        )
        .await
        .unwrap();
    let mut queue = PostgresOutbox::connect(database.url()).await.unwrap();
    let pending = queue.next_pending().await.unwrap().unwrap();
    assert_eq!(pending.event.game_id, game);
    assert_eq!(pending.event.user_id, black);
    assert_eq!(pending.event.request_id, request);
    queue.mark_published(&pending).await.unwrap();
    assert!(queue.next_pending().await.unwrap().is_none());
}

#[tokio::test]
async fn malformed_ledgers_and_orphaned_outbox_tables_fail_closed() {
    for ddl in [
        "CREATE TABLE schema_version (id INTEGER); INSERT INTO schema_version VALUES (0)",
        "CREATE TABLE schema_version (id INTEGER, version TEXT); INSERT INTO schema_version VALUES (0, 'invalid')",
        "CREATE TABLE outbox_events (id INTEGER)",
    ] {
        let database = TestDatabase::new().await;
        let url = database.url();
        let raw = Database::connect(url).await.unwrap();
        raw.execute_unprepared(ddl).await.unwrap();
        assert!(
            matches!(SeaOrmStore::connect(url).await, Err(Error::Corrupt)),
            "{ddl}"
        );
    }
}
