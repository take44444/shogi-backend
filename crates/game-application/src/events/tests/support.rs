use super::*;

pub(super) fn receipt(action: Action, result: Option<GameResult>) -> Receipt {
    Receipt {
        request: MutationRequest {
            request_id: RequestId::new("request").unwrap(),
            user_id: UserId::new("black").unwrap(),
            expected_seq: 0,
            action,
        },
        acceptance: Acceptance {
            seq: 1,
            in_check: result.is_some(),
            result,
        },
    }
}
pub(super) fn game() -> GameId {
    GameId::new("game").unwrap()
}

pub(super) struct Queue {
    pub(super) events: VecDeque<CommittedEvent>,
    pub(super) mark_failure: bool,
    pub(super) committed_mark: bool,
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
pub(super) struct Sink {
    pub(super) observed: Arc<Mutex<Vec<u64>>>,
    pub(super) fail: bool,
}
#[async_trait]
impl NotificationPublisher for Sink {
    async fn publish(&mut self, event: &CommittedEvent) -> Result<(), PublishError> {
        self.observed.lock().unwrap().push(event.seq);
        if std::mem::take(&mut self.fail) {
            Err(PublishError("uncertain send".into()))
        } else {
            Ok(())
        }
    }
}
