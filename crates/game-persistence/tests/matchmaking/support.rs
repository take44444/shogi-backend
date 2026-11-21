use super::*;

pub(super) struct Fixture {
    pub(super) _db: TestDatabase,
    pub(super) store: Arc<SeaOrmStore>,
    pub(super) raw: DatabaseConnection,
}
impl Fixture {
    pub(super) async fn new() -> Self {
        let db = TestDatabase::new().await;
        let store = Arc::new(SeaOrmStore::connect(db.url()).await.unwrap());
        let raw = Database::connect(db.url()).await.unwrap();
        Self {
            _db: db,
            store,
            raw,
        }
    }
    pub(super) async fn count(&self, table: &str) -> i64 {
        self.raw
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                format!("SELECT count(*) AS n FROM {table}"),
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "n")
            .unwrap()
    }
    pub(super) async fn join(&self, user: &str, pool: Pool) -> TicketId {
        let id = TicketId::new();
        assert!(self.store.join(uid(user), id, pool).await.unwrap().created);
        id
    }
    pub(super) async fn ready(&self, user: &str) {
        let Participation::Active(ticket) = self.store.current(&uid(user)).await.unwrap() else {
            panic!("expected an active match");
        };
        self.store.ready(&uid(user), ticket.id).await.unwrap();
    }
}
pub(super) async fn active(store: &SeaOrmStore, user: &UserId) -> Ticket {
    let Participation::Active(ticket) = store.current(user).await.unwrap() else {
        panic!("expected active participation for {user:?}");
    };
    ticket
}

pub(super) fn uid(s: &str) -> UserId {
    UserId::new(s).unwrap()
}
pub(super) const POOL: Pool = Pool::TenMinute30;
