use super::*;

pub(super) fn id() -> GameId {
    GameId::new("game").unwrap()
}
pub(super) fn user(s: &str) -> UserId {
    UserId::new(s).unwrap()
}
pub(super) fn command() -> MakeMove {
    MakeMove {
        request_id: RequestId::new("move").unwrap(),
        expected_seq: 0,
        user_id: user("black"),
        movement: "7g7f".parse().unwrap(),
    }
}
pub(super) async fn setup(
    url: &str,
) -> (
    Arc<SeaOrmStore>,
    pg_test_support::GameFixture,
    DatabaseConnection,
) {
    let store = Arc::new(SeaOrmStore::connect(url).await.unwrap());
    let service = pg_test_support::GameFixture::new(store.clone(), url);
    service
        .seed(
            id(),
            Participants::new(user("black"), user("white")).unwrap(),
        )
        .await
        .unwrap();
    (store, service, Database::connect(url).await.unwrap())
}
pub(super) async fn count(db: &DatabaseConnection, table: &str) -> i64 {
    db.query_one_raw(Statement::from_string(
        DbBackend::Postgres,
        format!("SELECT COUNT(*) AS count FROM {table}"),
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "count")
    .unwrap()
}

pub(super) struct Uncertain(pub(super) Arc<SeaOrmStore>);
#[async_trait]
impl GameRepository for Uncertain {
    async fn expired_games(&self, limit: u32) -> Result<Vec<GameId>, Error> {
        self.0.expired_games(limit).await
    }
    async fn begin_game_update(
        &self,
        id: &GameId,
        terminal: bool,
    ) -> Result<Box<dyn game_application::LockedGameUpdate>, Error> {
        Ok(Box::new(UncertainUpdate(
            self.0.begin_game_update(id, terminal).await?,
        )))
    }

    async fn load_game_for_viewer(
        &self,
        id: &GameId,
        viewer: &UserId,
    ) -> Result<game_application::ViewerGameRecord, Error> {
        self.0.load_game_for_viewer(id, viewer).await
    }
    async fn view_metadata(
        &self,
        id: &GameId,
        viewer: &UserId,
    ) -> Result<game_application::ViewMetadata, Error> {
        self.0.view_metadata(id, viewer).await
    }

    async fn load_game(&self, id: &GameId) -> Result<GameRecord, Error> {
        self.0.load_game(id).await
    }
    async fn version(&self, id: &GameId) -> Result<u64, Error> {
        self.0.version(id).await
    }
}

struct UncertainUpdate(Box<dyn game_application::LockedGameUpdate>);
#[async_trait]
impl game_application::LockedGameUpdate for UncertainUpdate {
    fn locked_record(&self) -> &GameRecord {
        self.0.locked_record()
    }
    async fn decision_time(&mut self) -> Result<game_application::Timestamp, Error> {
        self.0.decision_time().await
    }
    fn terminal_allowed(&self) -> bool {
        self.0.terminal_allowed()
    }
    async fn rollback(self: Box<Self>) -> Result<(), Error> {
        self.0.rollback().await
    }
    async fn commit(self: Box<Self>, change: &StagedGameChange) -> Result<(), Error> {
        self.0.commit(change).await?;
        Err(Error::Unavailable)
    }
}
