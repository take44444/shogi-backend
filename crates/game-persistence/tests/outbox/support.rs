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
    async fn commit_change(&self, change: &StagedGameChange) -> Result<(), Error> {
        self.0.commit_change(change).await?;
        Err(Error::Unavailable)
    }
}
