//! Durable admission contracts and pure, bilateral rating selection.

use crate::{ApplicationError, GameId, UserId};
use async_trait::async_trait;
use rating_domain::{Pool, RatingState};
use shogi_domain::Player;
use uuid::Uuid;

/// An owner-scoped, client generated retry identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TicketId(Uuid);
impl TicketId {
    /// Parses a UUID without accepting arbitrary application IDs.
    pub fn parse(value: &str) -> Result<Self, uuid::Error> {
        Uuid::parse_str(value).map(Self)
    }
    /// Generates a new request identity.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl Default for TicketId {
    fn default() -> Self {
        Self::new()
    }
}
impl std::fmt::Display for TicketId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// A strictly positive sequence bounded by PostgreSQL BIGINT.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeartbeatSequence(i64);
impl HeartbeatSequence {
    /// Accepts positive sequences through `i64::MAX`.
    pub fn new(value: i64) -> Option<Self> {
        (value > 0).then_some(Self(value))
    }
    /// The validated wire and storage value.
    pub fn get(self) -> i64 {
        self.0
    }
}

/// Persisted reason why this request never entered the queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefusalReason {
    /// Another ticket is waiting in either pool.
    AlreadyWaiting,
    /// Another ticket is awaiting confirmation in either pool.
    AlreadyMatched,
    /// Another ticket is playing in either pool.
    AlreadyPlaying,
    /// This pool's preceding rating job still owns its reservation.
    RatingPending,
}
impl RefusalReason {
    /// Stable public and storage code.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyWaiting => "already_waiting",
            Self::AlreadyMatched => "already_matched",
            Self::AlreadyPlaying => "already_playing",
            Self::RatingPending => "rating_pending",
        }
    }
}

/// Server-owned waiting identity. Timestamps are database epoch milliseconds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Waiting {
    /// Independent rating pool.
    pub pool: Pool,
    /// Complete committed rating captured at admission.
    pub rating: RatingState,
    /// Immutable admission time.
    pub enqueued_at: i64,
    /// Current exclusive expiry boundary.
    pub expires_at: i64,
    /// Last accepted heartbeat sequence, initially zero.
    pub heartbeat_seq: i64,
}
/// Exclusive ticket lifecycle. Terminal records remain durable for retries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TicketState {
    /// Eligible for pairing. Newer heartbeats renew its lease.
    Waiting(Waiting),
    /// A prospective match awaits both owners' confirmation.
    Matched {
        /// Prospective identity, unavailable through game APIs until playing.
        game_id: GameId,
        /// Permanent side.
        side: Player,
        /// This owner's durable confirmation.
        ready: bool,
        /// Exclusive database epoch millisecond deadline for both owners.
        ready_deadline: i64,
    },
    /// A committed game owns the user's global participation.
    Playing {
        /// Committed game.
        game_id: GameId,
        /// Permanent side.
        side: Player,
    },
    /// Terminal game, independent of eventual rating application.
    Finished {
        /// Committed game.
        game_id: GameId,
        /// Permanent side.
        side: Player,
    },
    /// The shared confirmation deadline elapsed without a playable game.
    Voided {
        /// Prospective identity retained for retries.
        game_id: GameId,
        /// Assigned side retained for retries.
        side: Player,
    },
    /// Cancelled, including a cancellation received before join.
    Cancelled,
    /// Waiting lifetime elapsed.
    Expired,
    /// Durable admission rejection.
    Refused(RefusalReason),
}
/// One durable request, scoped to its owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ticket {
    /// Authenticated owner.
    pub user: UserId,
    /// Retry identity.
    pub id: TicketId,
    /// Immutable requested pool. Only a preceding cancellation has no pool.
    pub pool: Option<Pool>,
    /// Current exclusive state.
    pub state: TicketState,
}
/// A join result distinguishes new acceptance from an exact retry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinOutcome {
    /// Stored current ticket, including durable refusals.
    pub ticket: Ticket,
    /// True only for a newly admitted waiting request.
    pub created: bool,
}
/// An owner-visible current participation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Participation {
    /// No waiting, matched or playing ticket. Rating may still be pending.
    Idle,
    /// One active ticket across both pools.
    Active(Ticket),
}
/// Admission errors for which no successful response can be claimed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MatchmakingError {
    /// No ticket belonging to this owner exists.
    NotFound,
    /// The same identity was submitted with a different pool.
    PoolConflict,
    /// Storage unavailable or commit response uncertain. Retry the same identity.
    Unavailable,
    /// Persisted state violates the ticket or rating contract.
    Corrupt,
}
impl From<ApplicationError> for MatchmakingError {
    fn from(error: ApplicationError) -> Self {
        if error == ApplicationError::Corrupt {
            Self::Corrupt
        } else {
            Self::Unavailable
        }
    }
}
impl std::fmt::Display for MatchmakingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for MatchmakingError {}

/// Owns atomic admission, matching, and durable retry recovery.
#[async_trait]
pub trait MatchmakingRepository: Send + Sync {
    /// Stores a join or durable refusal, or returns the prior request.
    async fn join(
        &self,
        user: UserId,
        ticket: TicketId,
        pool: Pool,
    ) -> Result<JoinOutcome, MatchmakingError>;
    /// Returns this owner's active ticket, expiring waiting and matched state first.
    async fn current(&self, user: &UserId) -> Result<Participation, MatchmakingError>;
    /// Returns the owned ticket's lifecycle state, including matched and terminal states,
    /// after expiring waiting and matched state. Only a live waiting ticket with a
    /// strictly newer sequence renews its lease. Duplicate or older sequences do not
    /// extend it. Matched heartbeats neither extend readiness nor confirm readiness.
    async fn heartbeat(
        &self,
        user: &UserId,
        ticket: TicketId,
        sequence: HeartbeatSequence,
    ) -> Result<Ticket, MatchmakingError>;
    /// Cancels waiting state or creates a cancellation tombstone.
    async fn cancel(&self, user: &UserId, ticket: TicketId) -> Result<Ticket, MatchmakingError>;
    /// Confirms a live match, starting its game only when both owners are ready.
    /// Other states return unchanged, without recording advance confirmation.
    async fn ready(&self, user: &UserId, ticket: TicketId) -> Result<Ticket, MatchmakingError>;
    /// Expires waiting and matched state and forms at most one pair in the pool.
    /// The returned identity is prospective until both owners confirm.
    async fn tick(&self, pool: Pool) -> Result<Option<GameId>, MatchmakingError>;
}

/// Allowed difference after complete 30-second periods, capped at 400 Elo.
pub fn window(enqueued_at: i64, now: i64) -> i128 {
    let periods = (i128::from(now) - i128::from(enqueued_at)).max(0) / 30_000;
    (100 + periods * 50).min(400)
}
/// Returns the oldest compatible anchor and its nearest compatible opponent.
/// Callers may pass any order. Expired tickets and different pools cannot pair.
pub fn select_pair(tickets: &[Ticket], now: i64) -> Option<(&Ticket, &Ticket)> {
    let mut waiting: Vec<_> = tickets
        .iter()
        .filter_map(|ticket| match &ticket.state {
            TicketState::Waiting(state) if state.expires_at > now => Some((ticket, state)),
            _ => None,
        })
        .collect();
    waiting.sort_by_key(|(ticket, state)| (state.enqueued_at, ticket.id, ticket.user.as_str()));
    for (anchor, a) in &waiting {
        let opponent = waiting
            .iter()
            .filter_map(|(other, b)| {
                let difference =
                    (i128::from(a.rating.effective()) - i128::from(b.rating.effective())).abs();
                (anchor.user != other.user
                    && a.pool == b.pool
                    && difference <= window(a.enqueued_at, now)
                    && difference <= window(b.enqueued_at, now))
                .then_some((difference, b.enqueued_at, other.id, *other))
            })
            .min_by_key(|(difference, time, id, other)| {
                (*difference, *time, *id, other.user.as_str())
            });
        if let Some((_, _, _, other)) = opponent {
            return Some((anchor, other));
        }
    }
    None
}

#[cfg(test)]
#[path = "trace/matchmaking-pairing/selection/tests.rs"]
mod tests;
