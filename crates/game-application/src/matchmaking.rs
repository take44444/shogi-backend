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
    /// Rejects zero, negative and exhausted client sequences.
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
    /// Still eligible for pairing and heartbeats.
    Waiting(Waiting),
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
    /// No waiting or playing ticket. Rating may still be pending.
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
    /// Returns this owner's active ticket, expiring stale waiting state first.
    async fn current(&self, user: &UserId) -> Result<Participation, MatchmakingError>;
    /// Reads this owner's request, expiring stale waiting state first.
    async fn ticket(&self, user: &UserId, ticket: TicketId) -> Result<Ticket, MatchmakingError>;
    /// Extends only a live waiting ticket with a strictly newer sequence.
    async fn heartbeat(
        &self,
        user: &UserId,
        ticket: TicketId,
        sequence: HeartbeatSequence,
    ) -> Result<Ticket, MatchmakingError>;
    /// Cancels waiting state or creates a cancellation tombstone.
    async fn cancel(&self, user: &UserId, ticket: TicketId) -> Result<Ticket, MatchmakingError>;
    /// Expires waiting state and atomically forms at most one pair in the pool.
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
            .min_by_key(|(difference, time, id, _)| (*difference, *time, *id));
        if let Some((_, _, _, other)) = opponent {
            return Some((anchor, other));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ticket(n: u128, rating: i64, time: i64) -> Ticket {
        Ticket {
            user: UserId::new(format!("u{n}")).unwrap(),
            id: TicketId(Uuid::from_u128(n)),
            pool: Some(Pool::TenMinute30),
            state: TicketState::Waiting(Waiting {
                pool: Pool::TenMinute30,
                rating: RatingState::from_parts(Some(rating), 1).unwrap(),
                enqueued_at: time,
                expires_at: 1_000_000,
                heartbeat_seq: 0,
            }),
        }
    }
    #[test]
    fn widening_is_bilateral_and_includes_boundary() {
        assert_eq!(window(0, 29_999), 100);
        assert_eq!(window(0, 30_000), 150);
        assert_eq!(window(i64::MIN, i64::MAX), 400);
        assert!(select_pair(&[ticket(1, 1500, 0), ticket(2, 1650, 30_000)], 30_000).is_none());
        assert!(select_pair(&[ticket(1, 1500, 0), ticket(2, 1650, 0)], 30_000).is_some());
        assert_eq!(RatingState::Unrated.effective(), 1500);
    }
    #[test]
    fn skips_unmatched_head_and_prefers_nearest_then_oldest_then_id() {
        let candidates = [
            ticket(1, 3000, 0),
            ticket(4, 1520, 3),
            ticket(3, 1480, 2),
            ticket(2, 1500, 1),
        ];
        let (a, b) = select_pair(&candidates, 3).unwrap();
        assert_eq!((a.id, b.id), (candidates[3].id, candidates[2].id));
        let candidates = [ticket(3, 1520, 0), ticket(2, 1500, 0), ticket(1, 1480, 0)];
        let (a, b) = select_pair(&candidates, 0).unwrap();
        assert_eq!((a.id, b.id), (candidates[2].id, candidates[1].id));
    }
    #[test]
    fn extremes_expiry_and_pool_separation() {
        assert!(select_pair(&[ticket(1, i64::MIN, 0), ticket(2, i64::MAX, 0)], 180_000).is_none());
        assert!(select_pair(&[ticket(1, 1500, 0), ticket(2, 1500, 0)], 1_000_000).is_none());
        let mut other = ticket(2, 1500, 0);
        if let TicketState::Waiting(state) = &mut other.state {
            state.pool = Pool::Byoyomi30;
        }
        assert!(select_pair(&[ticket(1, 1500, 0), other], 0).is_none());
        assert!(HeartbeatSequence::new(0).is_none());
        assert!(HeartbeatSequence::new(-1).is_none());
        assert_eq!(HeartbeatSequence::new(i64::MAX).unwrap().get(), i64::MAX);
    }
}
