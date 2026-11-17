#![cfg(test)]

use game_application::matchmaking::{
    HeartbeatSequence, MatchmakingError, MatchmakingRepository, Participation, RefusalReason,
    TicketId, TicketState,
};
use game_application::rating::{Pool, RatingState};
use game_application::{GameId, GameRepository, GameService, RequestId, Resign, UserId};
use game_persistence::SeaOrmStore;
use pg_test_support::TestDatabase;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};
use std::sync::Arc;

struct Fixture {
    _db: TestDatabase,
    store: Arc<SeaOrmStore>,
    raw: DatabaseConnection,
}
impl Fixture {
    async fn new() -> Self {
        let db = TestDatabase::new().await;
        let store = Arc::new(SeaOrmStore::connect(db.url()).await.unwrap());
        let raw = Database::connect(db.url()).await.unwrap();
        Self {
            _db: db,
            store,
            raw,
        }
    }
    async fn count(&self, table: &str) -> i64 {
        self.raw
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                format!("SELECT count(*) AS n FROM {table}"),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "n")
            .unwrap()
    }
    async fn join(&self, user: &str, pool: Pool) -> TicketId {
        let id = TicketId::new();
        assert!(self.store.join(uid(user), id, pool).await.unwrap().created);
        id
    }
    async fn ready(&self, user: &str) {
        let Participation::Active(ticket) = self.store.current(&uid(user)).await.unwrap() else {
            panic!("expected an active match");
        };
        self.store.ready(&uid(user), ticket.id).await.unwrap();
    }
}
fn uid(s: &str) -> UserId {
    UserId::new(s).unwrap()
}
const POOL: Pool = Pool::TenMinute30;

#[path = "../matchmaking-tickets/admission/selection.rs"]
mod matchmaking_tickets_admission;

#[path = "durable-tick/tests.rs"]
mod matchmaking_pairing_durable_tick;

#[path = "readiness/tests.rs"]
mod matchmaking_pairing_readiness;

#[path = "../matchmaking-tickets/heartbeat/tests.rs"]
mod matchmaking_tickets_heartbeat;

#[path = "../matchmaking-tickets/cancel/tests.rs"]
mod matchmaking_tickets_cancel;

#[path = "../game-command-recovery/durable-commit/selection.rs"]
mod game_command_recovery_durable_commit;
