use super::*;

pub(super) struct Fixture {
    pub(super) database: TestDatabase,
    pub(super) store: Arc<SeaOrmStore>,
    pub(super) jobs: Arc<SeaOrmRatingJobs>,
    pub(super) raw: DatabaseConnection,
}
impl Fixture {
    pub(super) async fn new() -> Self {
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
    pub(super) async fn create(&self, id: &str, pool: Pool) {
        pg_test_support::seed_rated(
            self.database.url(),
            &game(id),
            &players("black", "white"),
            pool,
        )
        .await
        .unwrap();
    }
    pub(super) async fn finish(&self, id: &str) {
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
    pub(super) async fn count(&self, table: &str) -> i64 {
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
    pub(super) async fn reader(&self) -> PostgresGameRecords {
        PostgresGameRecords::connect(&self.database.reader_url().await)
            .await
            .unwrap()
    }
}
pub(super) fn game(id: &str) -> GameId {
    GameId::new(id).unwrap()
}
pub(super) fn user(id: &str) -> UserId {
    UserId::new(id).unwrap()
}
pub(super) fn players(black: &str, white: &str) -> Participants {
    Participants::new(user(black), user(white)).unwrap()
}
pub(super) fn owner(id: &str) -> WorkerId {
    WorkerId::new(id).unwrap()
}
pub(super) fn policy(lease: u64, attempts: u32) -> WorkerPolicy {
    WorkerPolicy::new(
        Duration::from_millis(lease),
        NonZeroU32::new(attempts).unwrap(),
        Duration::ZERO,
    )
    .unwrap()
}
pub(super) fn change(claim: &ClaimedRatingJob) -> RatingChange {
    let i = claim.input().unwrap();
    rating_domain::calculate(i.game.black_start, i.game.white_start, i.winner).unwrap()
}

pub(super) struct Uncertain {
    pub(super) inner: Arc<SeaOrmRatingJobs>,
    pub(super) first: AtomicBool,
    pub(super) after_commit: bool,
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

pub(super) async fn wait_for_blocked_transactions(f: &Fixture, minimum: i64) {
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
