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

/// A committed event identified by `(game_id, seq, event.kind())`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommittedEvent {
    /// The game that produced the event.
    pub game_id: GameId,
    /// The accepted request, for correlation.
    pub request_id: RequestId,
    /// The participant who issued the command.
    pub user_id: UserId,
    /// The accepted command sequence.
    pub seq: u64,
    /// The immutable application fact.
    pub event: DomainEvent,
}
impl Receipt {
    /// Derives the canonical event batch, in move-then-finish order.
    ///
    /// Returns [`ApplicationError::Corrupt`] for inconsistent sequence or result.
    pub fn events(&self, game_id: &GameId) -> Result<Vec<CommittedEvent>, ApplicationError> {
        if self.expected_seq.checked_add(1) != Some(self.acceptance.seq)
            || self.acceptance.seq > i64::MAX as u64
        {
            return Err(ApplicationError::Corrupt);
        }
        let mut events = Vec::with_capacity(2);
        let mut push = |event| {
            events.push(CommittedEvent {
                game_id: game_id.clone(),
                request_id: self.request_id.clone(),
                user_id: self.user_id.clone(),
                seq: self.acceptance.seq,
                event,
            })
        };
        match self.action {
            Action::Move(movement) => {
                if self
                    .acceptance
                    .result
                    .is_some_and(|r| r.reason != FinishReason::Checkmate)
                {
                    return Err(ApplicationError::Corrupt);
                }
                push(DomainEvent::MoveCommitted {
                    movement,
                    in_check: self.acceptance.in_check,
                    result: self.acceptance.result,
                });
            }
            Action::Resign => {
                if !self
                    .acceptance
                    .result
                    .is_some_and(|r| r.reason == FinishReason::Resignation)
                {
                    return Err(ApplicationError::Corrupt);
                }
            }
        }
        if let Some(result) = self.acceptance.result {
            push(DomainEvent::GameFinished { result });
        }
        Ok(events)
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

/// Publishes application events through a transport boundary.
#[async_trait]
pub trait EventPublisher: Send {
    /// Success means socket flush and an exact event echo from the server succeeded.
    /// Core NATS does not acknowledge subscriber delivery.
    async fn publish(&mut self, event: &CommittedEvent) -> Result<(), PublishError>;
}

/// An outbox step failed before it could confirm marking.
#[derive(Debug)]
pub enum DispatchError {
    /// A queue read or mark failed, possibly after a mark committed.
    Repository(ApplicationError),
    /// Publishing or flushing failed, possibly after sending.
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
impl<R: OutboxRepository, P: EventPublisher> OutboxPublisher<R, P> {
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
mod tests {
    use super::*;
    use crate::Acceptance;
    use shogi_domain::Player;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    fn receipt(action: Action, result: Option<GameResult>) -> Receipt {
        Receipt {
            request_id: RequestId::new("request").unwrap(),
            user_id: UserId::new("black").unwrap(),
            expected_seq: 0,
            action,
            acceptance: Acceptance {
                seq: 1,
                in_check: result.is_some(),
                result,
            },
        }
    }
    fn game() -> GameId {
        GameId::new("game").unwrap()
    }
    #[test]
    fn canonical_move_mate_and_resignation_events() {
        let movement = "7g7f".parse().unwrap();
        let normal = receipt(Action::Move(movement), None)
            .events(&game())
            .unwrap();
        assert_eq!(normal.len(), 1);
        assert_eq!(
            normal[0].event,
            DomainEvent::MoveCommitted {
                movement,
                in_check: false,
                result: None
            }
        );
        let mate = GameResult {
            winner: Player::Black,
            reason: FinishReason::Checkmate,
        };
        let terminal = receipt(Action::Move(movement), Some(mate))
            .events(&game())
            .unwrap();
        assert_eq!(terminal.len(), 2);
        assert_eq!(terminal[0].seq, terminal[1].seq);
        assert_eq!(terminal[0].event.kind(), EventKind::MoveCommitted);
        assert_eq!(
            terminal[1].event,
            DomainEvent::GameFinished { result: mate }
        );
        let result = GameResult {
            reason: FinishReason::Resignation,
            ..mate
        };
        let end = receipt(Action::Resign, Some(result))
            .events(&game())
            .unwrap();
        assert_eq!(end.len(), 1);
        assert_eq!(end[0].event, DomainEvent::GameFinished { result });
        for bad in [
            receipt(Action::Resign, None),
            receipt(Action::Resign, Some(mate)),
            receipt(Action::Move(movement), Some(result)),
        ] {
            assert_eq!(bad.events(&game()), Err(ApplicationError::Corrupt));
        }
    }

    struct Queue {
        events: VecDeque<CommittedEvent>,
        mark_failure: bool,
        committed_mark: bool,
    }
    #[async_trait]
    impl OutboxRepository for Queue {
        type Cursor = u64;
        async fn next_pending(&mut self) -> Result<Option<OutboxItem<u64>>, ApplicationError> {
            Ok(self.events.front().cloned().map(|event| OutboxItem {
                cursor: event.seq,
                event,
            }))
        }
        async fn mark_published(&mut self, item: &OutboxItem<u64>) -> Result<(), ApplicationError> {
            assert_eq!(self.events.front().unwrap().seq, item.cursor);
            if self.mark_failure {
                self.mark_failure = false;
                if self.committed_mark {
                    self.events.pop_front();
                }
                return Err(ApplicationError::Unavailable);
            }
            self.events.pop_front();
            Ok(())
        }
    }
    struct Sink {
        observed: Arc<Mutex<Vec<u64>>>,
        fail: bool,
    }
    #[async_trait]
    impl EventPublisher for Sink {
        async fn publish(&mut self, event: &CommittedEvent) -> Result<(), PublishError> {
            self.observed.lock().unwrap().push(event.seq);
            if std::mem::take(&mut self.fail) {
                Err(PublishError("uncertain send".into()))
            } else {
                Ok(())
            }
        }
    }
    #[tokio::test]
    async fn failed_publish_and_uncertain_mark_preserve_identity_and_order() {
        for (send_failure, mark_failure, committed_mark, expected) in [
            (true, false, false, vec![1, 1, 2]),
            (false, true, false, vec![1, 1, 2]),
            (false, true, true, vec![1, 2]),
        ] {
            let first = receipt(Action::Move("7g7f".parse().unwrap()), None)
                .events(&game())
                .unwrap()
                .remove(0);
            let mut second = first.clone();
            second.seq = 2;
            let observed = Arc::default();
            let queue = Queue {
                events: [first, second].into(),
                mark_failure,
                committed_mark,
            };
            let sink = Sink {
                observed: Arc::clone(&observed),
                fail: send_failure,
            };
            let mut runner = OutboxPublisher::new(queue, sink);
            assert!(runner.publish_next().await.is_err());
            while runner.publish_next().await.unwrap() {}
            assert_eq!(*observed.lock().unwrap(), expected);
        }
    }
}
