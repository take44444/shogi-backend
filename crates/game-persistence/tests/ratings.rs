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

#[tokio::test]
async fn participation_is_global_and_refused_retries_remain_refused() {
    let f = Fixture::new().await;
    let first = TicketId::new();
    f.store
        .join(user("black"), first, Pool::TenMinute30)
        .await
        .unwrap();
    let second = TicketId::new();
    let refused = f
        .store
        .join(user("black"), second, Pool::Byoyomi30)
        .await
        .unwrap();
    assert_eq!(
        refused.ticket.state,
        TicketState::Refused(RefusalReason::AlreadyWaiting)
    );
    f.store.cancel(&user("black"), first).await.unwrap();
    assert_eq!(
        f.store
            .join(user("black"), second, Pool::Byoyomi30)
            .await
            .unwrap(),
        refused
    );
}

#[tokio::test]
async fn terminal_receipt_outbox_and_rating_job_commit_or_rollback_together() {
    let f = Fixture::new().await;
    f.create("rated", Pool::TenMinute30).await;
    f.raw.execute_unprepared("CREATE FUNCTION reject_rating_job() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected job failure'; END $$; CREATE TRIGGER reject_rating_job BEFORE INSERT ON rating_jobs FOR EACH ROW EXECUTE FUNCTION reject_rating_job()").await.unwrap();
    let games = GameService::new(f.store.clone());
    let command = Resign {
        request_id: RequestId::new("finish").unwrap(),
        expected_seq: 0,
        user_id: user("white"),
    };
    assert_eq!(
        games.resign(&game("rated"), command.clone()).await,
        Err(ApplicationError::Unavailable)
    );
    assert_eq!(f.store.version(&game("rated")).await.unwrap(), 0);
    for table in ["journal", "outbox_events", "rating_jobs"] {
        assert_eq!(f.count(table).await, 0)
    }
    f.raw
        .execute_unprepared("DROP TRIGGER reject_rating_job ON rating_jobs")
        .await
        .unwrap();
    let accepted = games.resign(&game("rated"), command.clone()).await.unwrap();
    assert_eq!(
        games.resign(&game("rated"), command).await.unwrap(),
        accepted
    );
    for table in ["journal", "outbox_events", "rating_jobs"] {
        assert_eq!(f.count(table).await, 1)
    }
    pg_test_support::GameFixture::new(f.store.clone(), f.database.url())
        .seed(game("casual"), players("black", "white"))
        .await
        .unwrap();
    f.finish("casual").await;
    assert_eq!(f.count("rating_jobs").await, 1);
    assert!(
        pg_test_support::rated_identity(f.database.url(), &game("casual"))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn completed_replay_is_read_only_and_never_releases_the_next_games_slots() {
    let f = Fixture::new().await;
    f.create("one", Pool::TenMinute30).await;
    f.finish("one").await;
    let policy = policy(30000, 3);
    let claim = f.jobs.claim(&owner("one"), &policy).await.unwrap().unwrap();
    let change = change(&claim);
    assert_eq!(
        f.jobs.apply(&claim, &change).await.unwrap(),
        ApplyOutcome::Applied
    );
    f.create("two", Pool::TenMinute30).await;
    let next = pg_test_support::rated_identity(f.database.url(), &game("two"))
        .await
        .unwrap();
    assert_eq!(next.black_start.rating(), Some(1516));
    assert_eq!(next.white_start.rating(), Some(1484));
    assert_eq!(
        f.jobs.apply(&claim, &change).await.unwrap(),
        ApplyOutcome::AlreadyApplied
    );
    assert_eq!(
        f.jobs
            .fail(&claim, JobFailure::Transient, &policy)
            .await
            .unwrap(),
        TickOutcome::LostClaim
    );
    assert_eq!(f.count("rating_slots").await, 2);
    assert_eq!(f.count("rating_applications").await, 1);
    let reader = f.reader().await;
    let snapshot = reader.ratings(&user("black")).await.unwrap();
    assert_eq!(snapshot.ten_minute_30.rating(), Some(1516));
    assert_eq!(snapshot.ten_minute_30.completed(), 1);
    assert_eq!(snapshot.byoyomi_30, RatingState::Unrated);
    assert_eq!(
        reader
            .ratings(&user("unknown"))
            .await
            .unwrap()
            .ten_minute_30,
        RatingState::Unrated
    );
    assert_eq!(
        pg_test_support::seed_rated(
            f.database.url(),
            &game("blocked"),
            &players("black", "third"),
            Pool::TenMinute30
        )
        .await,
        Err(ApplicationError::RatingPoolBusy)
    );
}

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
#[tokio::test]
async fn worker_reconciles_errors_before_and_after_a_real_application_commit() {
    for after_commit in [false, true] {
        let f = Fixture::new().await;
        f.create("uncertain", Pool::TenMinute30).await;
        f.finish("uncertain").await;
        let repository = Arc::new(Uncertain {
            inner: f.jobs.clone(),
            first: AtomicBool::new(true),
            after_commit,
        });
        let worker = RatingWorker::new(repository, policy(30000, 3));
        assert_eq!(
            worker.tick(&owner("uncertain")).await.unwrap(),
            if after_commit {
                TickOutcome::AlreadyApplied
            } else {
                TickOutcome::Applied
            }
        );
        assert_eq!(
            worker.tick(&owner("uncertain")).await.unwrap(),
            TickOutcome::Idle
        );
        assert_eq!(f.count("rating_applications").await, 1);
        assert_eq!(f.count("rating_slots").await, 0);
    }
}

#[tokio::test]
async fn expired_claims_consume_budget_quarantine_and_recover_without_releasing_slots() {
    let f = Fixture::new().await;
    f.create("expiry", Pool::TenMinute30).await;
    f.finish("expiry").await;
    let policy = policy(30000, 2);
    let old = f.jobs.claim(&owner("old"), &policy).await.unwrap().unwrap();
    f.raw
        .execute_unprepared("UPDATE rating_jobs SET lease_until=0")
        .await
        .unwrap();
    assert_eq!(
        f.jobs
            .fail(&old, JobFailure::Transient, &policy)
            .await
            .unwrap(),
        TickOutcome::LostClaim
    );
    let new = f.jobs.claim(&owner("new"), &policy).await.unwrap().unwrap();
    assert!(new.generation() > old.generation());
    assert_eq!(
        f.jobs.apply(&old, &change(&old)).await.unwrap(),
        ApplyOutcome::LostClaim
    );
    assert_eq!(
        f.jobs
            .fail(&old, JobFailure::Corrupt, &policy)
            .await
            .unwrap(),
        TickOutcome::LostClaim
    );
    f.raw
        .execute_unprepared("UPDATE rating_jobs SET lease_until=0")
        .await
        .unwrap();
    assert!(
        f.jobs
            .claim(&owner("third"), &policy)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(f.count("rating_slots").await, 2);
    assert_eq!(f.count("rating_applications").await, 0);
    assert_eq!(
        f.jobs.retry_quarantined(&game("expiry")).await.unwrap(),
        RecoveryOutcome::Retried
    );
    assert_eq!(
        f.jobs.retry_quarantined(&game("expiry")).await.unwrap(),
        RecoveryOutcome::NotQuarantined
    );
    assert_eq!(
        f.jobs.retry_quarantined(&game("missing")).await.unwrap(),
        RecoveryOutcome::NotFound
    );
    assert_eq!(
        f.jobs
            .fail(&new, JobFailure::Transient, &policy)
            .await
            .unwrap(),
        TickOutcome::LostClaim
    );
    let recovered = f
        .jobs
        .claim(&owner("recovered"), &policy)
        .await
        .unwrap()
        .unwrap();
    assert!(recovered.generation() > new.generation());
    assert_eq!(
        f.jobs.apply(&recovered, &change(&recovered)).await.unwrap(),
        ApplyOutcome::Applied
    );
    assert_eq!(f.count("rating_slots").await, 0);
}

#[tokio::test]
async fn corrupt_current_state_quarantines_and_retry_requires_repair() {
    let f = Fixture::new().await;
    f.create("corrupt", Pool::TenMinute30).await;
    f.finish("corrupt").await;
    f.raw
        .execute_unprepared(
            "UPDATE rating_states SET rating=1600,completed=1 WHERE user_id='black'",
        )
        .await
        .unwrap();
    let worker = RatingWorker::new(f.jobs.clone(), policy(30000, 3));
    assert_eq!(
        worker.tick(&owner("worker")).await.unwrap(),
        TickOutcome::Quarantined
    );
    assert_eq!(f.count("rating_applications").await, 0);
    assert_eq!(f.count("rating_slots").await, 2);
    assert_eq!(
        worker.retry_quarantined(&game("corrupt")).await.unwrap(),
        RecoveryOutcome::Retried
    );
    assert_eq!(
        worker.tick(&owner("worker")).await.unwrap(),
        TickOutcome::Quarantined
    );
    f.raw
        .execute_unprepared(
            "UPDATE rating_states SET rating=NULL,completed=0 WHERE user_id='black'",
        )
        .await
        .unwrap();
    worker.retry_quarantined(&game("corrupt")).await.unwrap();
    assert_eq!(
        worker.tick(&owner("worker")).await.unwrap(),
        TickOutcome::Applied
    );
}

#[tokio::test]
async fn lease_expiring_while_waiting_for_state_lock_rolls_back_all_mutations() {
    let f = Fixture::new().await;
    f.create("wait", Pool::TenMinute30).await;
    f.finish("wait").await;
    let claim = f
        .jobs
        .claim(&owner("waiting"), &policy(300, 3))
        .await
        .unwrap()
        .unwrap();
    let delta = change(&claim);
    let held = f.raw.begin().await.unwrap();
    held.execute_unprepared("SELECT * FROM rating_states FOR UPDATE")
        .await
        .unwrap();
    let jobs = f.jobs.clone();
    let task = tokio::spawn(async move { jobs.apply(&claim, &delta).await });
    tokio::time::timeout(Duration::from_secs(3),async {
        loop {
            let n:i64=f.raw.query_one_raw(Statement::from_string(DbBackend::Postgres,"SELECT count(*) AS n FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock'")).await.unwrap().unwrap().try_get("","n").unwrap();
            if n>0 {break}tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.unwrap();
    f.raw
        .execute_unprepared("SELECT pg_sleep(0.35)")
        .await
        .unwrap();
    held.commit().await.unwrap();
    assert_eq!(task.await.unwrap().unwrap(), ApplyOutcome::LostClaim);
    assert_eq!(f.count("rating_applications").await, 0);
    assert_eq!(f.count("rating_slots").await, 2);
    assert_eq!(
        f.reader()
            .await
            .ratings(&user("black"))
            .await
            .unwrap()
            .ten_minute_30,
        RatingState::Unrated
    );
}

#[tokio::test]
async fn actual_reader_role_can_read_only_the_public_rating_table() {
    let f = Fixture::new().await;
    f.create("permissions", Pool::TenMinute30).await;
    let reader_url = f.database.reader_url().await;
    let mut options = sea_orm::ConnectOptions::new(reader_url);
    options.max_connections(1);
    let connection = Database::connect(options).await.unwrap();
    connection
        .execute_unprepared("SET default_transaction_read_only=off")
        .await
        .unwrap();
    for table in ["schema_version", "games", "journal", "rating_states"] {
        connection
            .query_all_raw(Statement::from_string(
                DbBackend::Postgres,
                format!("SELECT * FROM {table}"),
            ))
            .await
            .unwrap();
    }
    for table in [
        "rated_games",
        "rating_slots",
        "rating_jobs",
        "rating_applications",
        "matchmaking_tickets",
    ] {
        let error = connection
            .query_all_raw(Statement::from_string(
                DbBackend::Postgres,
                format!("SELECT * FROM {table}"),
            ))
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("permission denied"),
            "{table}: {error}"
        );
    }
    assert!(
        connection
            .execute_unprepared("UPDATE rating_states SET rating=1500,completed=1")
            .await
            .unwrap_err()
            .to_string()
            .contains("permission denied")
    );
}

#[tokio::test]
async fn both_pools_update_independently_from_their_frozen_start_states() {
    let f = Fixture::new().await;
    f.create("main-time", Pool::TenMinute30).await;
    f.finish("main-time").await;
    f.create("byoyomi", Pool::Byoyomi30).await;
    GameService::new(f.store.clone())
        .resign(
            &game("byoyomi"),
            Resign {
                request_id: RequestId::new("finish").unwrap(),
                expected_seq: 0,
                user_id: user("black"),
            },
        )
        .await
        .unwrap();
    let worker = RatingWorker::new(f.jobs.clone(), policy(30000, 3));
    assert_eq!(
        worker.tick(&owner("both")).await.unwrap(),
        TickOutcome::Applied
    );
    assert_eq!(
        worker.tick(&owner("both")).await.unwrap(),
        TickOutcome::Applied
    );
    let reader = f.reader().await;
    let snapshot = reader.ratings(&user("black")).await.unwrap();
    assert_eq!(snapshot.ten_minute_30.rating(), Some(1516));
    assert_eq!(snapshot.byoyomi_30.rating(), Some(1484));
    assert_eq!(snapshot.ten_minute_30.completed(), 1);
    assert_eq!(snapshot.byoyomi_30.completed(), 1);
    assert_eq!(f.count("rating_applications").await, 2);
    assert_eq!(f.count("rating_slots").await, 0);
}

#[tokio::test]
async fn concurrent_workers_acquire_disjoint_jobs_and_skip_a_locked_job() {
    let f = Fixture::new().await;
    f.create("locked", Pool::TenMinute30).await;
    f.finish("locked").await;
    f.create("available", Pool::Byoyomi30).await;
    f.finish("available").await;
    let held = f.raw.begin().await.unwrap();
    held.execute_unprepared("SELECT * FROM rating_jobs WHERE game_id='locked' FOR UPDATE")
        .await
        .unwrap();
    let acquired = tokio::time::timeout(
        Duration::from_secs(2),
        f.jobs.claim(&owner("worker"), &policy(30000, 3)),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    assert_eq!(acquired.game_id(), &game("available"));
    held.commit().await.unwrap();
    let first_owner = owner("one");
    let second_owner = owner("two");
    let claim_policy = policy(30000, 3);
    let (first, second) = tokio::join!(
        f.jobs.claim(&first_owner, &claim_policy),
        f.jobs.claim(&second_owner, &claim_policy)
    );
    let claims = [first.unwrap(), second.unwrap()];
    assert_eq!(claims.iter().filter(|c| c.is_some()).count(), 1);
    assert_eq!(
        claims.into_iter().flatten().next().unwrap().game_id(),
        &game("locked")
    );
}

#[tokio::test]
async fn transient_failure_obeys_backoff_and_quarantines_at_the_attempt_limit() {
    let f = Fixture::new().await;
    f.create("retry", Pool::TenMinute30).await;
    f.finish("retry").await;
    let policy = WorkerPolicy::new(
        Duration::from_secs(30),
        NonZeroU32::new(2).unwrap(),
        Duration::from_secs(30),
    )
    .unwrap();
    let first = f.jobs.claim(&owner("one"), &policy).await.unwrap().unwrap();
    assert_eq!(
        f.jobs
            .fail(&first, JobFailure::Transient, &policy)
            .await
            .unwrap(),
        TickOutcome::Deferred
    );
    assert!(
        f.jobs
            .claim(&owner("early"), &policy)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        f.jobs
            .fail(&first, JobFailure::Corrupt, &policy)
            .await
            .unwrap(),
        TickOutcome::LostClaim
    );
    f.raw
        .execute_unprepared("UPDATE rating_jobs SET available_at=0")
        .await
        .unwrap();
    let second = f.jobs.claim(&owner("two"), &policy).await.unwrap().unwrap();
    assert_eq!(
        f.jobs
            .fail(&second, JobFailure::Transient, &policy)
            .await
            .unwrap(),
        TickOutcome::Quarantined
    );
    assert!(
        f.jobs
            .claim(&owner("exhausted"), &policy)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(f.count("rating_slots").await, 2);
    assert_eq!(f.count("rating_applications").await, 0);
}

#[tokio::test]
async fn failed_ledger_insert_rolls_back_both_ratings_and_reservations() {
    let f = Fixture::new().await;
    f.create("atomic", Pool::TenMinute30).await;
    f.finish("atomic").await;
    let claim = f
        .jobs
        .claim(&owner("atomic"), &policy(30000, 3))
        .await
        .unwrap()
        .unwrap();
    f.raw.execute_unprepared("CREATE FUNCTION reject_application() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected ledger failure'; END $$; CREATE TRIGGER reject_application BEFORE INSERT ON rating_applications FOR EACH ROW EXECUTE FUNCTION reject_application()").await.unwrap();
    assert_eq!(
        f.jobs.apply(&claim, &change(&claim)).await,
        Err(RatingWorkerError::Unavailable)
    );
    assert_eq!(f.count("rating_applications").await, 0);
    assert_eq!(f.count("rating_slots").await, 2);
    let reader = f.reader().await;
    for who in ["black", "white"] {
        assert_eq!(
            reader.ratings(&user(who)).await.unwrap().ten_minute_30,
            RatingState::Unrated
        )
    }
    f.raw
        .execute_unprepared("DROP TRIGGER reject_application ON rating_applications")
        .await
        .unwrap();
    assert_eq!(
        f.jobs.apply(&claim, &change(&claim)).await.unwrap(),
        ApplyOutcome::Applied
    );
}

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

#[tokio::test]
async fn admission_waits_for_application_and_freezes_the_new_committed_states() {
    let f = Fixture::new().await;
    f.create("previous", Pool::TenMinute30).await;
    f.finish("previous").await;
    let claim = f
        .jobs
        .claim(&owner("applying"), &policy(30000, 3))
        .await
        .unwrap()
        .unwrap();
    let delta = change(&claim);
    let held = f.raw.begin().await.unwrap();
    held.execute_unprepared("SELECT * FROM rating_states FOR UPDATE")
        .await
        .unwrap();
    let jobs = f.jobs.clone();
    let applying = tokio::spawn(async move { jobs.apply(&claim, &delta).await });
    wait_for_blocked_transactions(&f, 1).await;
    let store = f.store.clone();
    let admission = tokio::spawn(async move {
        store
            .join(user("black"), TicketId::new(), Pool::TenMinute30)
            .await
    });
    wait_for_blocked_transactions(&f, 2).await;
    held.commit().await.unwrap();
    assert_eq!(applying.await.unwrap().unwrap(), ApplyOutcome::Applied);
    assert!(admission.await.unwrap().unwrap().created);
    f.store
        .join(user("white"), TicketId::new(), Pool::TenMinute30)
        .await
        .unwrap();
    let next = f.store.tick(Pool::TenMinute30).await.unwrap().unwrap();
    let identity = pg_test_support::rated_identity(f.database.url(), &next)
        .await
        .unwrap();
    let (black, white) = if identity.participants.black() == &user("black") {
        (identity.black_start, identity.white_start)
    } else {
        (identity.white_start, identity.black_start)
    };
    assert_eq!(black.rating(), Some(1516));
    assert_eq!(white.rating(), Some(1484));
    assert_eq!(black.completed(), 1);
    assert_eq!(white.completed(), 1);
    assert_eq!(f.count("rating_slots").await, 2);
}
