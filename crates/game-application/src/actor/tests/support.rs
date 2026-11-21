use super::*;

pub(super) struct RecordStore(
    pub(super) Arc<Mutex<GameRecord>>,
    pub(super) Arc<Mutex<Vec<StagedGameChange>>>,
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
        Arc::new(Mutex::new(GameRecord {
            time_control: crate::TimeControl::for_pool(crate::rating::Pool::TenMinute30),
            initial_clock: crate::ClockState::start(
                crate::TimeControl::for_pool(crate::rating::Pool::TenMinute30),
                crate::Timestamp::new(0).unwrap(),
            )
            .unwrap(),
            clock: crate::ClockState::start(
                crate::TimeControl::for_pool(crate::rating::Pool::TenMinute30),
                crate::Timestamp::new(0).unwrap(),
            )
            .unwrap(),
            server_now_ms: crate::Timestamp::new(0).unwrap(),
            id: id(),
            participants: Participants::new(
                UserId::new("black").unwrap(),
                UserId::new("white").unwrap(),
            )
            .unwrap(),
            version: 0,
            result: None,
            journal: vec![],
        })),
        Arc::default(),
    ))
}
#[async_trait]
impl GameRepository for RecordStore {
    async fn begin_game_update(
        &self,
        _: &GameId,
        terminal: bool,
    ) -> Result<Box<dyn crate::LockedGameUpdate>, ApplicationError> {
        Ok(Box::new(LockedRecord {
            record: self.0.lock().unwrap().clone(),
            stored: self.0.clone(),
            changes: self.1.clone(),
            terminal,
        }))
    }
    async fn expired_games(&self, _: u32) -> Result<Vec<GameId>, ApplicationError> {
        Ok(vec![])
    }

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
}

struct LockedRecord {
    record: GameRecord,
    stored: Arc<Mutex<GameRecord>>,
    changes: Arc<Mutex<Vec<StagedGameChange>>>,
    terminal: bool,
}
#[async_trait]
impl crate::LockedGameUpdate for LockedRecord {
    fn locked_record(&self) -> &GameRecord {
        &self.record
    }
    async fn decision_time(&mut self) -> Result<Timestamp, ApplicationError> {
        Timestamp::new(0)
    }
    fn terminal_allowed(&self) -> bool {
        self.terminal
    }
    async fn rollback(self: Box<Self>) -> Result<(), ApplicationError> {
        Ok(())
    }
    async fn commit(self: Box<Self>, change: &StagedGameChange) -> Result<(), ApplicationError> {
        let mut record = self.stored.lock().unwrap();
        if record.version != change.entry().expected_seq() {
            return Err(ApplicationError::SequenceMismatch {
                expected: change.entry().expected_seq(),
                actual: record.version,
            });
        }
        record.version = change.entry().acceptance().seq;
        record.result = change.entry().acceptance().result;
        record.clock = change.entry().acceptance().clock;
        record.journal.push(change.entry().clone());
        self.changes.lock().unwrap().push(change.clone());
        Ok(())
    }
}
