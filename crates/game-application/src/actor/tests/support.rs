use super::*;

pub(super) struct RecordStore(
    pub(super) Mutex<GameRecord>,
    pub(super) Mutex<Vec<StagedGameChange>>,
);
pub(super) fn id() -> GameId {
    GameId::new("lifecycle").unwrap()
}
pub(super) fn receipt() -> MutationRequest {
    MutationRequest {
        request_id: RequestId::new("request").unwrap(),
        user_id: UserId::new("black").unwrap(),
        expected_seq: 0,
        action: Action::Move("7g7f".parse().unwrap()),
    }
}
pub(super) fn store() -> Arc<RecordStore> {
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
