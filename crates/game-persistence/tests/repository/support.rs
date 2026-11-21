use super::*;

pub(super) fn user(value: &str) -> UserId {
    UserId::new(value).unwrap()
}
pub(super) fn id() -> GameId {
    GameId::new("game").unwrap()
}
pub(super) fn participants() -> Participants {
    Participants::new(user("black"), user("white")).unwrap()
}
pub(super) fn movement(request: &str, expected_seq: u64, user_id: &str, usi: &str) -> MakeMove {
    MakeMove {
        request_id: RequestId::new(request).unwrap(),
        expected_seq,
        user_id: user(user_id),
        movement: usi.parse().unwrap(),
    }
}
pub(super) fn resign() -> Resign {
    Resign {
        request_id: RequestId::new("end").unwrap(),
        expected_seq: 5,
        user_id: user("black"),
    }
}
pub(super) async fn setup(url: &str) -> (Arc<SeaOrmStore>, pg_test_support::GameFixture) {
    let store = Arc::new(SeaOrmStore::connect(url).await.unwrap());
    let service = pg_test_support::GameFixture::new(store.clone(), url);
    service.seed(id(), participants()).await.unwrap();
    (store, service)
}

pub(super) struct FaultStore {
    pub(super) inner: Arc<SeaOrmStore>,
    pub(super) mode: AtomicU8,
    pub(super) unreadable: Arc<AtomicBool>,
}
#[async_trait]
impl GameRepository for FaultStore {
    async fn expired_games(&self, limit: u32) -> Result<Vec<GameId>, Error> {
        self.inner.expired_games(limit).await
    }
    async fn begin_game_update(
        &self,
        id: &GameId,
        terminal: bool,
    ) -> Result<Box<dyn game_application::LockedGameUpdate>, Error> {
        if self.unreadable.load(Ordering::SeqCst) {
            return Err(Error::Unavailable);
        }
        Ok(Box::new(FaultUpdate {
            locked: self.inner.begin_game_update(id, terminal).await?,
            inner: self.inner.clone(),
            mode: self.mode.swap(0, Ordering::SeqCst),
            unreadable: self.unreadable.clone(),
        }))
    }

    async fn load_game_for_viewer(
        &self,
        id: &GameId,
        viewer: &UserId,
    ) -> Result<game_application::ViewerGameRecord, Error> {
        self.inner.load_game_for_viewer(id, viewer).await
    }
    async fn view_metadata(
        &self,
        id: &GameId,
        viewer: &UserId,
    ) -> Result<game_application::ViewMetadata, Error> {
        self.inner.view_metadata(id, viewer).await
    }

    async fn load_game(&self, id: &GameId) -> Result<GameRecord, Error> {
        if self.unreadable.load(Ordering::SeqCst) {
            Err(Error::Unavailable)
        } else {
            self.inner.load_game(id).await
        }
    }
    async fn version(&self, id: &GameId) -> Result<u64, Error> {
        if self.unreadable.load(Ordering::SeqCst) {
            Err(Error::Unavailable)
        } else {
            self.inner.version(id).await
        }
    }
}

pub(super) async fn integer_query(db: &sea_orm::DatabaseConnection, sql: &str) -> i64 {
    db.query_one_raw(sea_orm::Statement::from_string(
        sea_orm::DbBackend::Postgres,
        sql,
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "value")
    .unwrap()
}

struct FaultUpdate {
    locked: Box<dyn game_application::LockedGameUpdate>,
    inner: Arc<SeaOrmStore>,
    mode: u8,
    unreadable: Arc<AtomicBool>,
}
#[async_trait]
impl game_application::LockedGameUpdate for FaultUpdate {
    fn locked_record(&self) -> &GameRecord {
        self.locked.locked_record()
    }
    async fn decision_time(&mut self) -> Result<game_application::Timestamp, Error> {
        self.locked.decision_time().await
    }
    fn terminal_allowed(&self) -> bool {
        self.locked.terminal_allowed()
    }
    async fn rollback(self: Box<Self>) -> Result<(), Error> {
        self.locked.rollback().await
    }
    async fn commit(self: Box<Self>, change: &StagedGameChange) -> Result<(), Error> {
        if self.mode == 1 {
            self.locked.rollback().await?;
            return Err(Error::Unavailable);
        }
        if self.mode == 4 {
            self.locked.rollback().await?;
            let mut locked = self.inner.begin_game_update(change.game_id(), true).await?;
            let record = locked.locked_record().clone();
            let mut different = match change.entry() {
                game_application::JournalEntry::UserCommand(r) => r.clone(),
                _ => panic!("expected user command"),
            };
            different.request.user_id = user("white");
            different.request.action = Action::Resign;
            different.acceptance.in_check = false;
            different.acceptance.result = Some(shogi_domain::GameResult {
                winner: Player::Black,
                reason: shogi_domain::FinishReason::Resignation,
            });
            different.evaluated_at = locked.decision_time().await?;
            different.acceptance.clock = record.clock.advance(
                record.time_control,
                Player::Black,
                different.evaluated_at,
                None,
            )?;
            locked
                .commit(&StagedGameChange::new(change.game_id().clone(), different)?)
                .await?;
        } else {
            self.locked.commit(change).await?;
        }
        if self.mode == 3 {
            self.unreadable.store(true, Ordering::SeqCst);
        }
        if self.mode >= 2 {
            Err(Error::Unavailable)
        } else {
            Ok(())
        }
    }
}
