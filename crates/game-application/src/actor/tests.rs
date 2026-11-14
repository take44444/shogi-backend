use super::*;
use crate::{
    CommittedEvent, DomainEvent, GameService, MAILBOX_CAPACITY, MakeMove, UserId, ViewMetadata,
    ViewerGameRecord, Visibility,
};
use async_trait::async_trait;
use std::sync::Mutex;

struct RecordStore(Mutex<GameRecord>, Mutex<Vec<StagedGameChange>>);
fn id() -> GameId {
    GameId::new("lifecycle").unwrap()
}
fn receipt() -> MutationRequest {
    MutationRequest {
        request_id: RequestId::new("request").unwrap(),
        user_id: UserId::new("black").unwrap(),
        expected_seq: 0,
        action: Action::Move("7g7f".parse().unwrap()),
    }
}
fn store() -> Arc<RecordStore> {
    Arc::new(RecordStore(
        Mutex::new(GameRecord {
            id: id(),
            participants: Participants::new(
                UserId::new("black").unwrap(),
                UserId::new("white").unwrap(),
            )
            .unwrap(),
            version: 0,
            result: None,
            journal: vec![],
        }),
        Mutex::default(),
    ))
}
#[async_trait]
impl GameRepository for RecordStore {
    async fn load_game_for_viewer(
        &self,
        id: &GameId,
        viewer: &UserId,
    ) -> Result<ViewerGameRecord, ApplicationError> {
        let record = self.load_game(id).await?;
        let access = self.view_metadata(id, viewer).await?;
        Ok(ViewerGameRecord { access, record })
    }
    async fn view_metadata(
        &self,
        id: &GameId,
        _: &UserId,
    ) -> Result<ViewMetadata, ApplicationError> {
        let record = self.load_game(id).await?;
        Ok(ViewMetadata {
            participants: record.participants,
            visibility: Visibility::Public,
        })
    }

    async fn load_game(&self, _: &GameId) -> Result<GameRecord, ApplicationError> {
        Ok(self.0.lock().unwrap().clone())
    }
    async fn version(&self, id: &GameId) -> Result<u64, ApplicationError> {
        let record = self.0.lock().unwrap();
        if &record.id != id {
            return Err(ApplicationError::NotFound);
        }
        Ok(record.version)
    }
    async fn commit_change(&self, change: &StagedGameChange) -> Result<(), ApplicationError> {
        self.1.lock().unwrap().push(change.clone());
        let receipt = change.receipt();
        let mut record = self.0.lock().unwrap();
        record.version = receipt.acceptance.seq;
        record.result = receipt.acceptance.result;
        record.journal.push(receipt.clone());
        Ok(())
    }
}

#[tokio::test]
async fn actor_supplies_events_before_commit_change_and_skips_retries_and_rejections() {
    let store = store();
    let mut actor = Actor::new(id(), store.clone());
    let first = receipt();
    let accepted = actor.apply(first.clone()).await.unwrap();
    assert_eq!(actor.apply(first.clone()).await, Ok(accepted));
    assert_eq!(
        actor
            .apply(MutationRequest {
                request_id: RequestId::new("rejected").unwrap(),
                expected_seq: 1,
                ..first.clone()
            })
            .await,
        Err(ApplicationError::Domain(
            shogi_domain::CommandError::WrongTurn
        ))
    );
    let resignation = MutationRequest {
        request_id: RequestId::new("end").unwrap(),
        user_id: UserId::new("white").unwrap(),
        expected_seq: 1,
        action: Action::Resign,
    };
    let finished = actor.apply(resignation.clone()).await.unwrap();
    assert_eq!(actor.apply(resignation.clone()).await, Ok(finished));

    let supplied = store.1.lock().unwrap();
    assert_eq!(supplied.len(), 2);
    assert_eq!(supplied[0].game_id(), &id());
    assert_eq!(supplied[0].receipt().acceptance, accepted);
    assert_eq!(
        supplied[0].events(),
        &[CommittedEvent {
            game_id: id(),
            request_id: first.request_id,
            user_id: first.user_id,
            seq: accepted.seq,
            event: DomainEvent::MoveCommitted {
                movement: "7g7f".parse().unwrap(),
                in_check: accepted.in_check,
                result: accepted.result,
            },
        }]
    );
    assert_eq!(supplied[1].receipt().acceptance, finished);
    assert_eq!(
        supplied[1].events(),
        &[CommittedEvent {
            game_id: id(),
            request_id: resignation.request_id,
            user_id: resignation.user_id,
            seq: finished.seq,
            event: DomainEvent::GameFinished {
                result: finished.result.unwrap(),
            },
        }]
    );
}

#[test]
fn sequence_cap_is_portable() {
    assert_eq!(next_sequence(i64::MAX as u64 - 1), Ok(i64::MAX as u64));
    assert_eq!(
        next_sequence(i64::MAX as u64),
        Err(ApplicationError::SequenceExhausted)
    );
    assert_eq!(
        next_sequence(u64::MAX),
        Err(ApplicationError::SequenceExhausted)
    );
}

#[tokio::test]
async fn cancelled_reply_and_closed_mailbox_still_drain_commands() {
    let actor = Actor::new(id(), store());
    let (sender, receiver) = mpsc::channel(MAILBOX_CAPACITY);
    let (reply, response) = oneshot::channel();
    sender
        .send(Command::Mutate {
            receipt: receipt(),
            reply,
        })
        .await
        .unwrap();
    drop(response);
    let (reply, response) = oneshot::channel();
    sender
        .send(Command::Mutate {
            receipt: receipt(),
            reply,
        })
        .await
        .unwrap();
    let (snapshot_reply, snapshot_response) = oneshot::channel();
    sender
        .send(Command::Snapshot(snapshot_reply))
        .await
        .unwrap();
    drop(sender);
    let task = tokio::spawn(actor.run(receiver));
    assert_eq!(response.await.unwrap().unwrap().seq, 1);
    assert_eq!(snapshot_response.await.unwrap().unwrap().seq, 1);
    task.await.unwrap();
}

#[tokio::test]
async fn cancelled_call_after_enqueue_executes_and_before_enqueue_does_not() {
    for full in [false, true] {
        let store = store();
        let service = GameService::new(store.clone());
        let (sender, receiver) = mpsc::channel(MAILBOX_CAPACITY);
        service.games.lock().unwrap().insert(id(), sender.clone());
        if full {
            for _ in 0..MAILBOX_CAPACITY {
                let (reply, _) = oneshot::channel();
                sender.send(Command::Snapshot(reply)).await.unwrap();
            }
        }
        let game_id = id();
        let command = MakeMove {
            request_id: receipt().request_id,
            user_id: receipt().user_id,
            expected_seq: 0,
            movement: "7g7f".parse().unwrap(),
        };
        let mut call = Box::pin(service.make_move(&game_id, command.clone()));
        tokio::select! { biased; result = &mut call => panic!("actor has not started, got {result:?}"), _ = tokio::task::yield_now() => {} }
        assert_eq!(receiver.len(), if full { MAILBOX_CAPACITY } else { 1 });
        drop(call);
        let task = tokio::spawn(Actor::new(id(), store).run(receiver));
        assert_eq!(service.snapshot(&id()).await.unwrap().seq, u64::from(!full));
        assert_eq!(service.make_move(&id(), command).await.unwrap().seq, 1);
        drop(sender);
        drop(service);
        task.await.unwrap();
    }
}

#[tokio::test]
async fn closed_actor_is_recovered_from_store_and_last_service_drop_closes_senders() {
    let service = GameService::new(store());
    let (sender, receiver) = mpsc::channel(MAILBOX_CAPACITY);
    drop(receiver);
    service.games.lock().unwrap().insert(id(), sender);
    assert_eq!(service.snapshot(&id()).await.unwrap().seq, 0);
    let weak = service
        .games
        .lock()
        .unwrap()
        .get(&id())
        .unwrap()
        .downgrade();
    let clone = service.clone();
    drop(service);
    assert!(weak.upgrade().is_some());
    drop(clone);
    assert!(weak.upgrade().is_none());
}

#[tokio::test]
async fn unknown_games_never_register_actors() {
    let service = GameService::new(store());
    for index in 0..100 {
        let missing = GameId::new(format!("missing-{index}")).unwrap();
        assert_eq!(
            service.snapshot(&missing).await,
            Err(ApplicationError::NotFound)
        );
        let command = MakeMove {
            request_id: receipt().request_id,
            user_id: receipt().user_id,
            expected_seq: 0,
            movement: "7g7f".parse().unwrap(),
        };
        assert_eq!(
            service.make_move(&missing, command).await,
            Err(ApplicationError::NotFound)
        );
    }
    assert!(service.games.lock().unwrap().is_empty());
}

#[tokio::test]
async fn mutation_errors_prioritize_request_conflict_then_sequence_mismatch_then_game_finished() {
    let service = GameService::new(store());
    let mut command = MakeMove {
        request_id: RequestId::new("oversized").unwrap(),
        user_id: UserId::new("black").unwrap(),
        expected_seq: u64::MAX,
        movement: "7g7f".parse().unwrap(),
    };
    assert_eq!(
        service.make_move(&id(), command.clone()).await,
        Err(ApplicationError::SequenceMismatch {
            expected: u64::MAX,
            actual: 0
        })
    );
    command.expected_seq = 0;
    assert_eq!(
        service.make_move(&id(), command.clone()).await.unwrap().seq,
        1
    );
    command.expected_seq = u64::MAX;
    assert_eq!(
        service.make_move(&id(), command.clone()).await,
        Err(ApplicationError::RequestConflict)
    );
    service
        .resign(
            &id(),
            crate::Resign {
                request_id: RequestId::new("finish").unwrap(),
                user_id: UserId::new("white").unwrap(),
                expected_seq: 1,
            },
        )
        .await
        .unwrap();
    command.request_id = RequestId::new("after-finish").unwrap();
    assert_eq!(
        service.make_move(&id(), command.clone()).await,
        Err(ApplicationError::SequenceMismatch {
            expected: u64::MAX,
            actual: 2
        })
    );
    command.expected_seq = 2;
    assert_eq!(
        service.make_move(&id(), command).await,
        Err(ApplicationError::Domain(
            shogi_domain::CommandError::GameFinished
        ))
    );
}

#[tokio::test]
async fn exhausted_state_still_checks_expected_sequence_before_advancing() {
    let repository = store();
    let mut actor = Actor::new(id(), repository.clone());
    actor.refresh().await.unwrap();
    actor.state.as_mut().unwrap().seq = i64::MAX as u64;
    repository.0.lock().unwrap().version = i64::MAX as u64;
    let mut request = receipt();
    request.expected_seq = u64::MAX;
    assert_eq!(
        actor.apply(request.clone()).await,
        Err(ApplicationError::SequenceMismatch {
            expected: u64::MAX,
            actual: i64::MAX as u64
        })
    );
    request.expected_seq = i64::MAX as u64;
    assert_eq!(
        actor.apply(request).await,
        Err(ApplicationError::SequenceExhausted)
    );
}
