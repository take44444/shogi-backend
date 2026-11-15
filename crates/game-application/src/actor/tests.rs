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

include!(
    "../trace/game-command-recovery/actor-apply/actor_supplies_events_before_commit_change_and_skips_retries_and_rejections.rs"
);

include!("../trace/game-command-recovery/sequence-cap/sequence_cap_is_portable.rs");

include!(
    "../trace/game-command-recovery/mailbox/cancelled_reply_and_closed_mailbox_still_drain_commands.rs"
);

include!(
    "../trace/game-command-recovery/mailbox/cancelled_call_after_enqueue_executes_and_before_enqueue_does_not.rs"
);

include!(
    "../trace/game-command-recovery/mailbox/closed_actor_is_recovered_from_store_and_last_service_drop_closes_senders.rs"
);

include!("../trace/game-snapshots-access/snapshot-refresh/unknown_games_never_register_actors.rs");

include!(
    "../trace/game-command-recovery/actor-apply/mutation_errors_prioritize_request_conflict_then_sequence_mismatch_then_game_finished.rs"
);

include!(
    "../trace/game-command-recovery/sequence-cap/exhausted_state_still_checks_expected_sequence_before_advancing.rs"
);
