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
    pub(super) unreadable: AtomicBool,
}
#[async_trait]
impl GameRepository for FaultStore {
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
    async fn commit_change(&self, change: &StagedGameChange) -> Result<(), Error> {
        let mode = self.mode.swap(0, Ordering::SeqCst);
        if mode == 1 {
            return Err(Error::Unavailable);
        }
        if mode == 4 {
            let mut different = change.receipt().clone();
            different.request.user_id = user("white");
            different.request.action = Action::Resign;
            different.acceptance.result = Some(shogi_domain::GameResult {
                winner: Player::Black,
                reason: shogi_domain::FinishReason::Resignation,
            });
            let different = StagedGameChange::new(change.game_id().clone(), different)?;
            self.inner.commit_change(&different).await?;
        } else {
            self.inner.commit_change(change).await?;
        }
        if mode == 3 {
            self.unreadable.store(true, Ordering::SeqCst);
        }
        if mode >= 2 {
            Err(Error::Unavailable)
        } else {
            Ok(())
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
