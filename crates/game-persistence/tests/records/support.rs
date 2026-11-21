use super::*;

pub(super) struct Fixture {
    pub(super) _database: pg_test_support::TestDatabase,
    pub(super) reader_url: String,
    pub(super) games: pg_test_support::GameFixture,
    pub(super) raw: DatabaseConnection,
}
impl Fixture {
    pub(super) async fn new() -> Self {
        let database = pg_test_support::TestDatabase::new().await;
        let url = database.url().to_string();
        let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
        let games = pg_test_support::GameFixture::new(store, database.url());
        let raw = Database::connect(&url).await.unwrap();
        Self {
            reader_url: database.reader_url().await,
            _database: database,
            games,
            raw,
        }
    }
    pub(super) async fn completed(
        &self,
        id: &str,
        black: &str,
        white: &str,
        moves: usize,
        loser: &str,
        time: i64,
    ) {
        let id = WriteId::new(id).unwrap();
        self.games
            .seed(
                id.clone(),
                Participants::new(UserId::new(black).unwrap(), UserId::new(white).unwrap())
                    .unwrap(),
            )
            .await
            .unwrap();
        for (index, usi) in ["7g7f", "3c3d"].iter().take(moves).enumerate() {
            self.games
                .make_move(
                    &id,
                    MakeMove {
                        request_id: RequestId::new(format!("move-{index}")).unwrap(),
                        expected_seq: index as u64,
                        user_id: UserId::new(if index % 2 == 0 { black } else { white }).unwrap(),
                        movement: usi.parse().unwrap(),
                    },
                )
                .await
                .unwrap();
        }
        self.games
            .resign(
                &id,
                Resign {
                    request_id: RequestId::new("secret-request").unwrap(),
                    expected_seq: moves as u64,
                    user_id: UserId::new(loser).unwrap(),
                },
            )
            .await
            .unwrap();
        self.raw
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "UPDATE games SET finished_at = $1 WHERE id = $2",
                [time.into(), storage_id(id.as_str()).into()],
            ))
            .await
            .unwrap();
    }
    pub(super) async fn reader(&self) -> PostgresGameRecords {
        PostgresGameRecords::connect(&self.reader_url)
            .await
            .unwrap()
    }
}
pub(super) fn filters() -> Filters {
    Filters {
        participant: None,
        rule: None,
        winner: None,
        reason: None,
        from: 0,
        to: 100,
        min_ply: None,
        max_ply: None,
    }
}
pub(super) fn search(f: Filters, limit: u32, cursor: Option<&str>) -> Search {
    Search::new(f, limit, cursor).unwrap()
}
pub(super) fn ids(page: &game_record_application::Page) -> Vec<&str> {
    page.items.iter().map(|i| i.id().as_str()).collect()
}

pub(super) async fn current_rows(db: &DatabaseConnection) -> Vec<Vec<String>> {
    let mut state = Vec::new();
    for sql in [
        "SELECT json_build_array(id,black,white,version,winner,reason,rule,visibility,finished_at,final_move_ply)::text AS value FROM games WHERE id LIKE 'current-%' ORDER BY id",
        "SELECT json_build_array(game_id,seq,request_id,user_id,expected_seq,action,usi,in_check,winner,reason)::text AS value FROM journal WHERE game_id LIKE 'current-%' ORDER BY game_id,seq",
        "SELECT json_build_array(id,game_id,seq,event_kind,encoding_version,request_id,user_id,usi,in_check,winner,reason,published)::text AS value FROM outbox_events WHERE game_id LIKE 'current-%' ORDER BY id",
    ] {
        state.push(
            db.query_all_raw(Statement::from_string(DbBackend::Postgres, sql))
                .await
                .unwrap()
                .into_iter()
                .map(|row| row.try_get::<String>("", "value").unwrap())
                .collect(),
        );
    }
    state
}

pub(super) fn storage_id(value: &str) -> String {
    value
        .replace('\u{1}', "\u{1}\u{2}")
        .replace('\0', "\u{1}\u{1}")
}
