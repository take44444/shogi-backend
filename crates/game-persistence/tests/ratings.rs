use async_trait::async_trait;
use game_application::matchmaking::{MatchmakingRepository, RefusalReason, TicketId, TicketState};
use game_application::rating::{
    ApplyOutcome, ClaimedRatingJob, JobFailure, Pool, RatingChange, RatingJobRepository,
    RatingState, RatingWorker, RatingWorkerError, RecoveryOutcome, TickOutcome, WorkerId,
    WorkerPolicy,
};
use game_application::{
    ApplicationError, GameId, GameRepository, GameService, Participants, RequestId, Resign, UserId,
};
use game_persistence::{SeaOrmRatingJobs, SeaOrmStore};
use game_record_application::ratings::RatingRepository;
use game_record_persistence::PostgresGameRecords;
use pg_test_support::TestDatabase;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};
use std::{
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

struct Fixture {
    database: TestDatabase,
    store: Arc<SeaOrmStore>,
    jobs: Arc<SeaOrmRatingJobs>,
    raw: DatabaseConnection,
}
impl Fixture {
    async fn new() -> Self {
        let database = TestDatabase::new().await;
        let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
        let jobs = Arc::new(SeaOrmRatingJobs::connect(database.url()).await.unwrap());
        let raw = Database::connect(database.url()).await.unwrap();
        Self {
            database,
            store,
            jobs,
            raw,
        }
    }
    async fn create(&self, id: &str, pool: Pool) {
        pg_test_support::seed_rated(
            self.database.url(),
            &game(id),
            &players("black", "white"),
            pool,
        )
        .await
        .unwrap();
    }
    async fn finish(&self, id: &str) {
        GameService::new(self.store.clone())
            .resign(
                &game(id),
                Resign {
                    request_id: RequestId::new("finish").unwrap(),
                    expected_seq: 0,
                    user_id: user("white"),
                },
            )
            .await
            .unwrap();
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
    async fn reader(&self) -> PostgresGameRecords {
        PostgresGameRecords::connect(&self.database.reader_url().await)
            .await
            .unwrap()
    }
}
fn game(id: &str) -> GameId {
    GameId::new(id).unwrap()
}
fn user(id: &str) -> UserId {
    UserId::new(id).unwrap()
}
fn players(black: &str, white: &str) -> Participants {
    Participants::new(user(black), user(white)).unwrap()
}
fn owner(id: &str) -> WorkerId {
    WorkerId::new(id).unwrap()
}
fn policy(lease: u64, attempts: u32) -> WorkerPolicy {
    WorkerPolicy::new(
        Duration::from_millis(lease),
        NonZeroU32::new(attempts).unwrap(),
        Duration::ZERO,
    )
    .unwrap()
}
fn change(claim: &ClaimedRatingJob) -> RatingChange {
    let i = claim.input().unwrap();
    rating_domain::calculate(i.game.black_start, i.game.white_start, i.winner).unwrap()
}

include!(
    "trace/matchmaking-tickets/admission/participation_is_global_and_refused_retries_remain_refused.rs"
);

include!(
    "trace/rating-application/terminal-job/terminal_receipt_outbox_and_rating_job_commit_or_rollback_together.rs"
);

include!(
    "trace/rating-application/atomic-application/completed_replay_is_read_only_and_never_releases_the_next_games_slots.rs"
);

struct Uncertain {
    inner: Arc<SeaOrmRatingJobs>,
    first: AtomicBool,
    after_commit: bool,
}
#[async_trait]
impl RatingJobRepository for Uncertain {
    async fn claim(
        &self,
        o: &WorkerId,
        p: &WorkerPolicy,
    ) -> Result<Option<ClaimedRatingJob>, RatingWorkerError> {
        self.inner.claim(o, p).await
    }
    async fn apply(
        &self,
        c: &ClaimedRatingJob,
        r: &RatingChange,
    ) -> Result<ApplyOutcome, RatingWorkerError> {
        if self.first.swap(false, Ordering::SeqCst) {
            if self.after_commit {
                assert_eq!(self.inner.apply(c, r).await?, ApplyOutcome::Applied)
            }
            return Err(RatingWorkerError::Unavailable);
        }
        self.inner.apply(c, r).await
    }
    async fn fail(
        &self,
        c: &ClaimedRatingJob,
        f: JobFailure,
        p: &WorkerPolicy,
    ) -> Result<TickOutcome, RatingWorkerError> {
        self.inner.fail(c, f, p).await
    }
    async fn retry_quarantined(&self, g: &GameId) -> Result<RecoveryOutcome, RatingWorkerError> {
        self.inner.retry_quarantined(g).await
    }
}
include!(
    "trace/rating-application/worker-reconciliation/worker_reconciles_errors_before_and_after_a_real_application_commit.rs"
);

include!(
    "trace/rating-application/claims-fences/expired_claims_consume_budget_quarantine_and_recover_without_releasing_slots.rs"
);

include!(
    "trace/rating-application/quarantine-recovery/corrupt_current_state_quarantines_and_retry_requires_repair.rs"
);

include!(
    "trace/rating-application/claims-fences/lease_expiring_while_waiting_for_state_lock_rolls_back_all_mutations.rs"
);

include!(
    "trace/startup-schema-isolation/reader-permissions/actual_reader_role_can_read_only_the_public_rating_table.rs"
);

include!(
    "trace/rating-application/atomic-application/both_pools_update_independently_from_their_frozen_start_states.rs"
);

include!(
    "trace/rating-application/claims-fences/concurrent_workers_acquire_disjoint_jobs_and_skip_a_locked_job.rs"
);

include!(
    "trace/rating-application/quarantine-recovery/transient_failure_obeys_backoff_and_quarantines_at_the_attempt_limit.rs"
);

include!(
    "trace/rating-application/atomic-application/failed_ledger_insert_rolls_back_both_ratings_and_reservations.rs"
);

async fn wait_for_blocked_transactions(f: &Fixture, minimum: i64) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let count: i64 = f.raw.query_one_raw(Statement::from_string(DbBackend::Postgres,
                "SELECT count(*) AS n FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock'"))
                .await.unwrap().unwrap().try_get("", "n").unwrap();
            if count >= minimum { break; }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.unwrap();
}

include!(
    "trace/matchmaking-tickets/admission/admission_waits_for_application_and_freezes_the_new_committed_states.rs"
);

#[cfg(test)]
#[path = "../../../scripts/traceability_model.rs"]
mod trace_support;
