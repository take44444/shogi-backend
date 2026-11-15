use game_application::matchmaking::{
    HeartbeatSequence, MatchmakingError, MatchmakingRepository, Participation, RefusalReason,
    TicketId, TicketState,
};
use game_application::rating::{Pool, RatingState};
use game_application::{GameRepository, GameService, RequestId, Resign, UserId};
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
}
fn uid(s: &str) -> UserId {
    UserId::new(s).unwrap()
}
const POOL: Pool = Pool::TenMinute30;

include!(
    "trace/matchmaking-tickets/admission/durable_retry_refusal_and_cancel_before_join_survive_reconnect.rs"
);

include!(
    "trace/matchmaking-pairing/durable-tick/cross_pool_joins_and_multiple_matchers_have_one_durable_winner.rs"
);

include!(
    "trace/matchmaking-pairing/durable-tick/pair_failure_rolls_back_and_terminal_failure_preserves_both_active_tickets.rs"
);

include!(
    "trace/matchmaking-tickets/heartbeat/heartbeat_retries_do_not_extend_and_expired_waiting_never_revives.rs"
);

include!("trace/matchmaking-pairing/durable-tick/expiry_after_rating_lock_wait_prevents_match.rs");

include!(
    "trace/matchmaking-tickets/cancel/cancellation_racing_a_match_has_one_committed_outcome.rs"
);

include!(
    "trace/matchmaking-pairing/durable-tick/widening_without_a_new_join_skips_an_incompatible_head.rs"
);

include!(
    "trace/matchmaking-pairing/durable-tick/lost_observations_recover_and_worker_retries_preserve_the_next_match.rs"
);

include!(
    "trace/game-command-recovery/durable-commit/missing_both_terminal_tickets_rolls_back_result_receipt_events_and_job.rs"
);

#[cfg(test)]
#[path = "../../../scripts/traceability_model.rs"]
mod trace_support;
