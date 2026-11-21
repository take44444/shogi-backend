use crate::{Action, ApplicationError, GameId, Receipt, RequestId, UserId};
use async_trait::async_trait;
use shogi_domain::{FinishReason, GameResult, Move};

/// Stable event identity component, distinct from a command sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventKind {
    /// A move was committed.
    MoveCommitted,
    /// A game reached a terminal result.
    GameFinished,
}

/// Application facts frozen when the receipt commits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainEvent {
    /// A legal move and its resulting position status.
    MoveCommitted {
        /// The accepted typed move.
        movement: Move,
        /// Whether the resulting side to move is checked.
        in_check: bool,
        /// The result if this move finished the game.
        result: Option<GameResult>,
    },
    /// A terminal game result.
    GameFinished {
        /// Winner and finish reason.
        result: GameResult,
    },
}
impl DomainEvent {
    /// Returns the event's stable identity component.
    pub fn kind(&self) -> EventKind {
        match self {
            Self::MoveCommitted { .. } => EventKind::MoveCommitted,
            Self::GameFinished { .. } => EventKind::GameFinished,
        }
    }
}

/// Origin of a committed event. System events have no user request identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventOrigin {
    /// A command accepted from a bound participant.
    User {
        /// Retained command identity.
        request_id: RequestId,
        /// Participant identity.
        user_id: UserId,
    },
    /// An autonomous timeout.
    System,
}

/// A committed event identified by `(game_id, seq, event.kind())`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommittedEvent {
    /// Clock frozen by the committed transition.
    pub clock: crate::ClockState,
    /// Database time when this transition was evaluated.
    pub evaluated_at: crate::Timestamp,
    /// The game that produced the event.
    pub game_id: GameId,
    /// User correlation or an autonomous system transition.
    pub origin: EventOrigin,
    /// The committed state transition sequence.
    pub seq: u64,
    /// The immutable application fact.
    pub event: DomainEvent,
}

/// An application-generated receipt and its canonical events, ready for atomic persistence.
///
/// Construction freezes the game, receipt, and event order together. Accessors expose
/// shared references so adapters can persist the supplied values without changing them.
///
/// ```compile_fail,E0616
/// use game_application::StagedGameChange;
/// fn replace_events(change: &mut StagedGameChange) {
///     change.events = Vec::new();
/// }
/// ```
///
/// ```compile_fail,E0594
/// use game_application::StagedGameChange;
/// fn change_receipt(change: &mut StagedGameChange) {
///     if let game_application::JournalEntry::UserCommand(receipt) = change.entry() {
///         receipt.acceptance.seq = 2;
///     }
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedGameChange {
    game_id: GameId,
    entry: crate::JournalEntry,
    events: Vec<CommittedEvent>,
}
impl StagedGameChange {
    /// Generates the canonical event batch in move-then-finish order.
    ///
    /// Validates sequence bounds, continuity, and action/result consistency.
    /// Board legality and participant membership are checked by the actor and repository.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationError::SequenceExhausted`] for out-of-range sequences or
    /// an expected sequence at `i64::MAX`. Returns [`ApplicationError::Corrupt`] for
    /// in-range nonconsecutive sequences or inconsistent action and result.
    pub fn new(game_id: GameId, receipt: Receipt) -> Result<Self, ApplicationError> {
        Self::from_entry(game_id, receipt.into())
    }

    /// Freezes a user or system transition and its canonical event batch.
    pub fn from_entry(
        game_id: GameId,
        entry: crate::JournalEntry,
    ) -> Result<Self, ApplicationError> {
        let acceptance = entry.acceptance();
        if entry.expected_seq() >= i64::MAX as u64 || acceptance.seq > i64::MAX as u64 {
            return Err(ApplicationError::SequenceExhausted);
        }
        if entry.expected_seq() + 1 != acceptance.seq {
            return Err(ApplicationError::Corrupt);
        }
        let origin = match entry.request() {
            Some(r) => EventOrigin::User {
                request_id: r.request_id.clone(),
                user_id: r.user_id.clone(),
            },
            None => EventOrigin::System,
        };
        let mut events = Vec::with_capacity(2);
        let mut push = |event| {
            events.push(CommittedEvent {
                game_id: game_id.clone(),
                origin: origin.clone(),
                seq: acceptance.seq,
                clock: acceptance.clock,
                evaluated_at: entry.evaluated_at(),
                event,
            })
        };
        match &entry {
            crate::JournalEntry::UserCommand(receipt) => match receipt.request.action {
                Action::Move(movement) => {
                    if acceptance
                        .result
                        .is_some_and(|r| r.reason != FinishReason::Checkmate)
                    {
                        return Err(ApplicationError::Corrupt);
                    }
                    push(DomainEvent::MoveCommitted {
                        movement,
                        in_check: acceptance.in_check,
                        result: acceptance.result,
                    });
                }
                Action::Resign => {
                    if !acceptance
                        .result
                        .is_some_and(|r| r.reason == FinishReason::Resignation)
                    {
                        return Err(ApplicationError::Corrupt);
                    }
                }
            },
            crate::JournalEntry::Timeout { loser, .. } => {
                if !acceptance.result.is_some_and(|r| {
                    r.reason == FinishReason::Timeout && r.winner == loser.opponent()
                }) {
                    return Err(ApplicationError::Corrupt);
                }
            }
        }
        if let Some(result) = acceptance.result {
            push(DomainEvent::GameFinished { result });
        }
        Ok(Self {
            game_id,
            entry,
            events,
        })
    }

    /// Returns the game whose receipt and events are staged.
    pub fn game_id(&self) -> &GameId {
        &self.game_id
    }

    /// Returns the user or system journal transition.
    pub fn entry(&self) -> &crate::JournalEntry {
        &self.entry
    }
    /// Returns the already generated events in canonical order.
    pub fn events(&self) -> &[CommittedEvent] {
        &self.events
    }
}

/// One pending event paired with an adapter-owned cursor.
pub struct OutboxItem<C> {
    /// Opaque cursor used only by the owning repository.
    pub cursor: C,
    /// The frozen committed event.
    pub event: CommittedEvent,
}

/// Exclusive, ordered access to committed events awaiting publication.
#[async_trait]
pub trait OutboxRepository: Send {
    /// Storage-specific cursor type, distinct from public event identity.
    type Cursor: Send + Sync;
    /// Returns the earliest pending row without holding a transaction open.
    async fn next_pending(&mut self) -> Result<Option<OutboxItem<Self::Cursor>>, ApplicationError>;
    /// Idempotently marks only this exact row after confirmed publication.
    async fn mark_published(
        &mut self,
        item: &OutboxItem<Self::Cursor>,
    ) -> Result<(), ApplicationError>;
}

/// A transport failure; the event must remain pending.
#[derive(Debug)]
pub struct PublishError(pub String);
impl std::fmt::Display for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for PublishError {}

/// Publishes committed events as transient notifications.
#[async_trait]
pub trait NotificationPublisher: Send {
    /// Success means the notification service accepted the event.
    /// It does not promise subscriber delivery, processing, or durable storage.
    /// An uncertain failure must leave the outbox item pending and may duplicate it.
    async fn publish(&mut self, event: &CommittedEvent) -> Result<(), PublishError>;
}

/// An outbox step failed before it could confirm marking.
#[derive(Debug)]
pub enum DispatchError {
    /// A queue read or mark failed, possibly after a mark committed.
    Repository(ApplicationError),
    /// Notification acceptance failed, possibly after sending.
    Publish(PublishError),
}
impl std::fmt::Display for DispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Repository(e) => write!(f, "outbox storage: {e}"),
            Self::Publish(e) => write!(f, "outbox transport: {e}"),
        }
    }
}
impl std::error::Error for DispatchError {}

/// Sequentially hands events to a transport and then marks their exact rows.
/// Errors retry the earliest pending identity; uncertain outcomes can duplicate it.
pub struct OutboxPublisher<R, P> {
    repository: R,
    publisher: P,
}
impl<R: OutboxRepository, P: NotificationPublisher> OutboxPublisher<R, P> {
    /// Takes ownership of the queue and transport to serialize dispatch.
    pub fn new(repository: R, publisher: P) -> Self {
        Self {
            repository,
            publisher,
        }
    }
    /// Publishes one event. Returns false when the queue is empty.
    ///
    /// Returns a storage or transport error without skipping pending rows.
    pub async fn publish_next(&mut self) -> Result<bool, DispatchError> {
        let Some(item) = self
            .repository
            .next_pending()
            .await
            .map_err(DispatchError::Repository)?
        else {
            return Ok(false);
        };
        self.publisher
            .publish(&item.event)
            .await
            .map_err(DispatchError::Publish)?;
        self.repository
            .mark_published(&item)
            .await
            .map_err(DispatchError::Repository)?;
        Ok(true)
    }
}

#[async_trait]
impl<R: OutboxRepository + ?Sized> OutboxRepository for &mut R {
    type Cursor = R::Cursor;
    async fn next_pending(&mut self) -> Result<Option<OutboxItem<Self::Cursor>>, ApplicationError> {
        (**self).next_pending().await
    }
    async fn mark_published(
        &mut self,
        item: &OutboxItem<Self::Cursor>,
    ) -> Result<(), ApplicationError> {
        (**self).mark_published(item).await
    }
}

#[cfg(test)]
mod tests;
