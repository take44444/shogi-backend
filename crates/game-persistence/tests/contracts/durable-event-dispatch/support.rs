#![cfg(test)]

use async_trait::async_trait;
use game_application::{
    ApplicationError as Error, DomainEvent, GameId, GameRecord, GameRepository, MakeMove,
    OutboxRepository, Participants, RequestId, Resign, StagedGameChange, UserId,
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
async fn setup(
    url: &str,
) -> (
    Arc<SeaOrmStore>,
    pg_test_support::GameFixture,
    DatabaseConnection,
) {
    let store = Arc::new(SeaOrmStore::connect(url).await.unwrap());
    let service = pg_test_support::GameFixture::new(store.clone(), url);
    service
        .seed(
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

struct Uncertain(Arc<SeaOrmStore>);
#[async_trait]
impl GameRepository for Uncertain {
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

#[path = "../startup-schema-isolation/contract-game-schema/outbox.rs"]
mod startup_schema_isolation_contract_game_schema;

#[path = "atomic-outbox/tests.rs"]
mod atomic_outbox;
#[path = "exclusive-queue/tests.rs"]
mod exclusive_queue;
