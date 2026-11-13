//! Rated admission identity and the durable worker lifecycle.
use crate::{GameId, Participants};
use async_trait::async_trait;
pub use rating_domain::{Pool, RatingChange, RatingState};
use shogi_domain::Player;
use std::{fmt, num::NonZeroU32, sync::Arc, time::Duration};

/// Immutable metadata used to reconcile uncertain rated creation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RatedGame {
    /// Immutable game identifier.
    pub game_id: GameId,
    /// Immutable side bindings.
    pub participants: Participants,
    /// Independent rating pool.
    pub pool: Pool,
    /// Black state frozen at admission.
    pub black_start: RatingState,
    /// White state frozen at admission.
    pub white_start: RatingState,
}
/// Frozen terminal input. A job never reads broker messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RatingInput {
    /// Immutable rated creation identity.
    pub game: RatedGame,
    /// Committed terminal sequence.
    pub final_seq: u64,
    /// Winner from the committed game result.
    pub winner: Player,
}
/// A validated process identity, independent of a lease generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerId(String);
impl WorkerId {
    /// Validates 1 through 128 bytes with no whitespace.
    pub fn new(value: impl Into<String>) -> Result<Self, RatingWorkerError> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 || value.chars().any(char::is_whitespace) {
            return Err(RatingWorkerError::InvalidConfiguration);
        }
        Ok(Self(value))
    }
    /// Borrows the validated identity.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
/// Storage-issued claim. Fields cannot change after acquisition.
#[derive(Clone, Debug)]
pub struct ClaimedRatingJob {
    game_id: GameId,
    owner: WorkerId,
    generation: i64,
    input: Result<RatingInput, JobFailure>,
}
impl ClaimedRatingJob {
    /// Reconstitute a storage claim after persisting its fence.
    pub fn new(
        game_id: GameId,
        owner: WorkerId,
        generation: i64,
        input: Result<RatingInput, JobFailure>,
    ) -> Result<Self, RatingWorkerError> {
        if generation <= 0
            || input.as_ref().is_ok_and(|i| {
                i.game.game_id != game_id || i.final_seq == 0 || i.final_seq > i64::MAX as u64
            })
        {
            return Err(RatingWorkerError::Corrupt);
        }
        Ok(Self {
            game_id,
            owner,
            generation,
            input,
        })
    }
    /// The game bound to this claim.
    pub fn game_id(&self) -> &GameId {
        &self.game_id
    }
    /// The owner bound to this claim.
    pub fn owner(&self) -> &WorkerId {
        &self.owner
    }
    /// The durable fencing generation.
    pub fn generation(&self) -> i64 {
        self.generation
    }
    /// Validated frozen input, or a corruption requiring quarantine.
    pub fn input(&self) -> Result<&RatingInput, JobFailure> {
        self.input.as_ref().map_err(|e| *e)
    }
}
/// Bounded retry and lease configuration.
#[derive(Clone, Debug)]
pub struct WorkerPolicy {
    lease: Duration,
    retry_limit: NonZeroU32,
    backoff: Duration,
}
impl WorkerPolicy {
    /// Validates positive lease and portable millisecond durations.
    pub fn new(
        lease: Duration,
        retry_limit: NonZeroU32,
        backoff: Duration,
    ) -> Result<Self, RatingWorkerError> {
        if lease.as_millis() == 0
            || lease.as_millis() > i64::MAX as u128
            || backoff.as_millis() > i64::MAX as u128
        {
            return Err(RatingWorkerError::InvalidConfiguration);
        }
        Ok(Self {
            lease,
            retry_limit,
            backoff,
        })
    }
    /// Claim lifetime in milliseconds.
    pub fn lease_millis(&self) -> i64 {
        self.lease.as_millis() as i64
    }
    /// Maximum acquisitions, including expired claims.
    pub fn retry_limit(&self) -> u32 {
        self.retry_limit.get()
    }
    /// Delay before a transient retry.
    pub fn backoff_millis(&self) -> i64 {
        self.backoff.as_millis() as i64
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Result of an atomic apply or read-only reconciliation.
pub enum ApplyOutcome {
    /// The rating transaction committed.
    Applied,
    /// A consistent completed ledger proves a prior commit.
    AlreadyApplied,
    /// The claim expired or another generation superseded it.
    LostClaim,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Result of processing one claim.
pub enum TickOutcome {
    /// No eligible job was acquired.
    Idle,
    /// The rating transaction committed.
    Applied,
    /// A consistent completed ledger proves a prior commit.
    AlreadyApplied,
    /// The fenced job was scheduled for retry.
    Deferred,
    /// The job remains reserved for operator recovery.
    Quarantined,
    /// The claim expired or another generation superseded it.
    LostClaim,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Result of the explicit quarantine recovery operation.
pub enum RecoveryOutcome {
    /// The quarantined job is pending with a fresh budget.
    Retried,
    /// The job is pending, claimed or completed and was unchanged.
    NotQuarantined,
    /// No durable rating job exists for this game.
    NotFound,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Reason to defer or quarantine a current claim.
pub enum JobFailure {
    /// A retryable storage failure.
    Transient,
    /// Persisted inputs or invariants are invalid.
    Corrupt,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Storage, data or runtime configuration failure.
pub enum RatingWorkerError {
    /// The database operation failed or has uncertain outcome.
    Unavailable,
    /// Persisted inputs or invariants are invalid.
    Corrupt,
    /// The worker configuration is outside its allowed range.
    InvalidConfiguration,
}
impl fmt::Display for RatingWorkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "rating storage unavailable",
            Self::Corrupt => "rating input or ledger corrupt",
            Self::InvalidConfiguration => "invalid rating worker configuration",
        })
    }
}
impl std::error::Error for RatingWorkerError {}
/// Transactions and fencing belong to the repository, not the worker runtime.
#[async_trait]
pub trait RatingJobRepository: Send + Sync {
    /// Acquires an eligible job, counting every expired claim toward the budget.
    async fn claim(
        &self,
        owner: &WorkerId,
        policy: &WorkerPolicy,
    ) -> Result<Option<ClaimedRatingJob>, RatingWorkerError>;
    /// Applies once under a valid fence, or reconciles a completed ledger read-only.
    async fn apply(
        &self,
        claim: &ClaimedRatingJob,
        change: &RatingChange,
    ) -> Result<ApplyOutcome, RatingWorkerError>;
    /// Defers or quarantines only the current unexpired claim.
    async fn fail(
        &self,
        claim: &ClaimedRatingJob,
        failure: JobFailure,
        policy: &WorkerPolicy,
    ) -> Result<TickOutcome, RatingWorkerError>;
    /// Resets quarantine without changing starts, ratings or reservations.
    async fn retry_quarantined(&self, game: &GameId) -> Result<RecoveryOutcome, RatingWorkerError>;
}
/// One short durable claim followed by pure calculation and atomic application.
pub struct RatingWorker {
    repository: Arc<dyn RatingJobRepository>,
    policy: WorkerPolicy,
}
impl RatingWorker {
    /// Constructs the worker around durable storage and retry policy.
    pub fn new(repository: Arc<dyn RatingJobRepository>, policy: WorkerPolicy) -> Self {
        Self { repository, policy }
    }
    /// Acquires, calculates and applies one durable job.
    pub async fn tick(&self, owner: &WorkerId) -> Result<TickOutcome, RatingWorkerError> {
        let Some(claim) = self.repository.claim(owner, &self.policy).await? else {
            return Ok(TickOutcome::Idle);
        };
        let change = claim.input().and_then(|i| {
            rating_domain::calculate(i.game.black_start, i.game.white_start, i.winner)
                .map_err(|_| JobFailure::Corrupt)
        });
        let change = match change {
            Ok(change) => change,
            Err(error) => return self.repository.fail(&claim, error, &self.policy).await,
        };
        let outcome = match self.repository.apply(&claim, &change).await {
            Ok(outcome) => outcome,
            Err(RatingWorkerError::Corrupt) => {
                return self
                    .repository
                    .fail(&claim, JobFailure::Corrupt, &self.policy)
                    .await;
            }
            Err(RatingWorkerError::Unavailable) => {
                match self.repository.apply(&claim, &change).await {
                    Ok(outcome) => outcome,
                    Err(RatingWorkerError::Unavailable) => {
                        return self
                            .repository
                            .fail(&claim, JobFailure::Transient, &self.policy)
                            .await;
                    }
                    Err(RatingWorkerError::Corrupt) => {
                        return self
                            .repository
                            .fail(&claim, JobFailure::Corrupt, &self.policy)
                            .await;
                    }
                    Err(error) => return Err(error),
                }
            }
            Err(error) => return Err(error),
        };
        Ok(match outcome {
            ApplyOutcome::Applied => TickOutcome::Applied,
            ApplyOutcome::AlreadyApplied => TickOutcome::AlreadyApplied,
            ApplyOutcome::LostClaim => TickOutcome::LostClaim,
        })
    }
    /// Explicitly retries one quarantined game without releasing its slots.
    pub async fn retry_quarantined(
        &self,
        game: &GameId,
    ) -> Result<RecoveryOutcome, RatingWorkerError> {
        self.repository.retry_quarantined(game).await
    }
}
