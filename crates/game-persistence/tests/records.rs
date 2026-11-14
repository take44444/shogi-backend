use game_application::{
    GameId as WriteId, GameRepository, GameService, MakeMove, Participants, RequestId, Resign,
    UserId,
};
use game_persistence::SeaOrmStore;
use game_record_application::{
    Error, Filters, FinishReason, GameId, MAX_HISTORY, PublicAction, RecordRepository, Rule, Search,
};
use game_record_persistence::PostgresGameRecords;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};
use shogi_domain::Player;
use std::sync::Arc;

struct Fixture {
    _database: pg_test_support::TestDatabase,
    reader_url: String,
    games: GameService,
    raw: DatabaseConnection,
}
impl Fixture {
    async fn new() -> Self {
        let database = pg_test_support::TestDatabase::new().await;
        let url = database.url().to_string();
        let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
        let games = GameService::new(store);
        let raw = Database::connect(&url).await.unwrap();
        Self {
            reader_url: database.reader_url().await,
            _database: database,
            games,
            raw,
        }
    }
    async fn completed(
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
            .create(
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
    async fn reader(&self) -> PostgresGameRecords {
        PostgresGameRecords::connect(&self.reader_url)
            .await
            .unwrap()
    }
}
fn filters() -> Filters {
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
fn search(f: Filters, limit: u32, cursor: Option<&str>) -> Search {
    Search::new(f, limit, cursor).unwrap()
}
fn ids(page: &game_record_application::Page) -> Vec<&str> {
    page.items.iter().map(|i| i.id().as_str()).collect()
}

#[tokio::test]
async fn public_completed_metadata_only_filters_and_stable_keysets() {
    let f = Fixture::new().await;
    f.completed("a", "alice", "bob", 0, "alice", 10).await;
    f.completed("b", "carol", "alice", 1, "alice", 20).await;
    f.completed("c", "alice", "dave", 2, "alice", 20).await;
    for id in ["hidden"] {
        f.completed(id, "alice", "bob", 0, "alice", 30).await;
    }
    f.raw
        .execute_unprepared("UPDATE games SET visibility='private' WHERE id='hidden'")
        .await
        .unwrap();
    f.games
        .create(
            WriteId::new("ongoing").unwrap(),
            Participants::new(UserId::new("alice").unwrap(), UserId::new("bob").unwrap()).unwrap(),
        )
        .await
        .unwrap();
    let reader = f.reader().await;
    let first = reader.search(&search(filters(), 1, None)).await.unwrap();
    assert_eq!(ids(&first), ["c"]);
    let second = reader
        .search(&search(filters(), 1, first.next_cursor.as_deref()))
        .await
        .unwrap();
    assert_eq!(ids(&second), ["b"]);
    let third = reader
        .search(&search(filters(), 1, second.next_cursor.as_deref()))
        .await
        .unwrap();
    assert_eq!(ids(&third), ["a"]);
    assert!(third.next_cursor.is_none());
    for (change, expected) in [
        (
            Filters {
                participant: Some(UserId::new("carol").unwrap()),
                ..filters()
            },
            vec!["b"],
        ),
        (
            Filters {
                participant: Some(UserId::new("alice").unwrap()),
                ..filters()
            },
            vec!["c", "b", "a"],
        ),
        (
            Filters {
                winner: Some(Player::Black),
                ..filters()
            },
            vec!["b"],
        ),
        (
            Filters {
                winner: Some(Player::White),
                ..filters()
            },
            vec!["c", "a"],
        ),
        (
            Filters {
                reason: Some(FinishReason::Checkmate),
                ..filters()
            },
            vec![],
        ),
        (
            Filters {
                reason: Some(FinishReason::Resignation),
                ..filters()
            },
            vec!["c", "b", "a"],
        ),
        (
            Filters {
                rule: Some(Rule::Standard),
                ..filters()
            },
            vec!["c", "b", "a"],
        ),
        (
            Filters {
                from: 20,
                to: 21,
                ..filters()
            },
            vec!["c", "b"],
        ),
        (
            Filters {
                from: 0,
                to: 20,
                ..filters()
            },
            vec!["a"],
        ),
        (
            Filters {
                min_ply: Some(1),
                max_ply: Some(1),
                ..filters()
            },
            vec!["b"],
        ),
        (
            Filters {
                max_ply: Some(0),
                ..filters()
            },
            vec!["a"],
        ),
        (
            Filters {
                participant: Some(UserId::new("alice").unwrap()),
                rule: Some(Rule::Standard),
                winner: Some(Player::White),
                reason: Some(FinishReason::Resignation),
                from: 20,
                to: 21,
                min_ply: Some(2),
                max_ply: Some(2),
            },
            vec!["c"],
        ),
    ] {
        assert_eq!(
            ids(&reader.search(&search(change, 100, None)).await.unwrap()),
            expected
        );
    }
    for id in ["hidden", "ongoing", "missing"] {
        assert_eq!(
            reader.detail(&GameId::new(id).unwrap()).await,
            Err(Error::NotFound)
        );
    }
    let detail = reader.detail(&GameId::new("c").unwrap()).await.unwrap();
    assert_eq!(detail.metadata.final_move_ply().get(), 2);
    assert_eq!(detail.metadata.final_seq().get(), 3);
    assert_eq!(
        detail
            .history
            .iter()
            .map(|o| o.seq.get())
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(detail.history.last().unwrap().action, PublicAction::Resign);
    let mut changed = filters();
    changed.winner = Some(Player::White);
    assert!(Search::new(changed, 1, first.next_cursor.as_deref()).is_err());
    f.completed("new", "alice", "bob", 0, "alice", 40).await;
    assert_eq!(
        ids(&reader
            .search(&search(filters(), 1, first.next_cursor.as_deref()))
            .await
            .unwrap()),
        ["b"]
    );
    f.raw
        .execute_unprepared("UPDATE games SET visibility='private' WHERE id='b'")
        .await
        .unwrap();
    assert_eq!(
        ids(&reader
            .search(&search(filters(), 1, first.next_cursor.as_deref()))
            .await
            .unwrap()),
        ["a"]
    );
}

#[tokio::test]
async fn writer_metadata_is_atomic_move_count_excludes_resign_and_retry_keeps_time() {
    let f = Fixture::new().await;
    f.completed("game", "black", "white", 1, "white", 50).await;
    let reader = f.reader().await;
    let before = reader.detail(&GameId::new("game").unwrap()).await.unwrap();
    f.games
        .resign(
            &WriteId::new("game").unwrap(),
            Resign {
                request_id: RequestId::new("secret-request").unwrap(),
                expected_seq: 1,
                user_id: UserId::new("white").unwrap(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        reader.detail(&GameId::new("game").unwrap()).await.unwrap(),
        before
    );
    f.games
        .create(
            WriteId::new("failed").unwrap(),
            Participants::new(UserId::new("black").unwrap(), UserId::new("white").unwrap())
                .unwrap(),
        )
        .await
        .unwrap();
    f.raw.execute_unprepared("CREATE FUNCTION fail_record() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'failure'; END $$; CREATE TRIGGER fail_record BEFORE INSERT ON journal FOR EACH ROW EXECUTE FUNCTION fail_record()").await.unwrap();
    assert!(
        f.games
            .resign(
                &WriteId::new("failed").unwrap(),
                Resign {
                    request_id: RequestId::new("r").unwrap(),
                    expected_seq: 0,
                    user_id: UserId::new("white").unwrap()
                }
            )
            .await
            .is_err()
    );
    let row = f
        .raw
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT version,final_move_ply,finished_at FROM games WHERE id='failed'",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "version").unwrap(), 0);
    assert_eq!(row.try_get::<i64>("", "final_move_ply").unwrap(), 0);
    assert_eq!(row.try_get::<Option<i64>>("", "finished_at").unwrap(), None);
}

#[tokio::test]
async fn corrupt_and_oversized_details_fail_without_partial_history() {
    for statement in [
        "UPDATE games SET final_move_ply=0",
        "UPDATE games SET version=3,final_move_ply=2",
        "UPDATE journal SET user_id='stranger' WHERE seq=1",
        "UPDATE journal SET usi='not-usi' WHERE seq=1",
        "UPDATE journal SET winner=2,reason=2 WHERE seq=1",
        "UPDATE journal SET winner=2,reason=2 WHERE seq=2",
        "UPDATE journal SET user_id='black' WHERE seq=2",
        "DELETE FROM outbox_events; DELETE FROM journal WHERE seq=1",
    ] {
        let f = Fixture::new().await;
        f.completed("game", "black", "white", 1, "white", 10).await;
        f.raw.execute_unprepared(statement).await.unwrap();
        assert_eq!(
            f.reader().await.detail(&GameId::new("game").unwrap()).await,
            Err(Error::Corrupt),
            "{statement}"
        );
    }
    let f = Fixture::new().await;
    f.completed("game", "black", "white", 0, "white", 10).await;
    f.raw
        .execute_unprepared(&format!(
            "UPDATE games SET version={},final_move_ply={}",
            MAX_HISTORY + 1,
            MAX_HISTORY
        ))
        .await
        .unwrap();
    assert_eq!(
        f.reader().await.detail(&GameId::new("game").unwrap()).await,
        Err(Error::HistoryTooLong)
    );
}

#[tokio::test]
async fn reader_does_not_create_or_accept_wrong_schema() {
    let empty = pg_test_support::TestDatabase::new().await;
    assert!(matches!(
        PostgresGameRecords::connect(empty.url()).await,
        Err(Error::Corrupt)
    ));
    let raw = Database::connect(empty.url()).await.unwrap();
    let tables = raw
        .query_all_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT table_name FROM information_schema.tables WHERE table_schema='public'",
        ))
        .await
        .unwrap();
    assert!(tables.is_empty());
    for sql in [
        "UPDATE schema_version SET version=3",
        "UPDATE schema_version SET version=99",
        "DELETE FROM schema_version",
        "DROP TABLE journal CASCADE",
        "ALTER TABLE games RENAME COLUMN finished_at TO missing",
    ] {
        let f = Fixture::new().await;
        f.raw.execute_unprepared(sql).await.unwrap();
        assert!(
            matches!(
                PostgresGameRecords::connect(&f.reader_url).await,
                Err(Error::Corrupt)
            ),
            "{sql}"
        );
    }
}

async fn current_rows(db: &DatabaseConnection) -> Vec<Vec<String>> {
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

#[tokio::test]
async fn reopening_preserves_current_records_history_and_outbox() {
    let database = pg_test_support::TestDatabase::new().await;
    let url = database.url().to_string();
    SeaOrmStore::connect(&url)
        .await
        .unwrap()
        .close()
        .await
        .unwrap();
    let db = Database::connect(&url).await.unwrap();
    db.execute_unprepared("INSERT INTO games VALUES
        ('current-finished','black','white',2,1,2,'standard','public',20,1),
        ('current-ongoing','black','white',1,NULL,NULL,'standard','public',NULL,1),
        ('current-private','black','white',2,1,2,'standard','private',30,1);
        INSERT INTO journal (game_id,seq,request_id,user_id,expected_seq,action,usi,in_check,winner,reason)
            SELECT id,1,'move','black',0,1,'7g7f',false,NULL,NULL FROM games;
        INSERT INTO journal (game_id,seq,request_id,user_id,expected_seq,action,usi,in_check,winner,reason)
            SELECT id,2,'end','white',1,2,NULL,false,1,2 FROM games WHERE version=2;
        INSERT INTO outbox_events (id,game_id,seq,event_kind,encoding_version,request_id,user_id,usi,in_check,winner,reason,published)
            SELECT row_number() OVER (ORDER BY game_id,seq)+100,game_id,seq,CASE action WHEN 1 THEN 0 ELSE 1 END,1,request_id,user_id,usi,CASE action WHEN 1 THEN in_check ELSE NULL END,winner,reason,CASE action WHEN 1 THEN true ELSE false END FROM journal;").await.unwrap();
    let before = current_rows(&db).await;
    let reader_url = database.reader_url().await;
    for _ in 0..2 {
        let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
        assert_eq!(current_rows(&db).await, before);
        let reader = PostgresGameRecords::connect(&reader_url).await.unwrap();
        assert_eq!(
            ids(&reader.search(&search(filters(), 30, None)).await.unwrap()),
            ["current-finished"]
        );
        assert_eq!(
            reader
                .detail(&GameId::new("current-finished").unwrap())
                .await
                .unwrap()
                .history
                .len(),
            2
        );
        assert_eq!(
            store
                .load_game(&WriteId::new("current-ongoing").unwrap())
                .await
                .unwrap()
                .version,
            1
        );
        reader.close().await;
        store.close().await.unwrap();
    }
    let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
    GameService::new(store.clone())
        .resign(
            &WriteId::new("current-ongoing").unwrap(),
            Resign {
                request_id: RequestId::new("later-end").unwrap(),
                expected_seq: 1,
                user_id: UserId::new("white").unwrap(),
            },
        )
        .await
        .unwrap();
    let reader = PostgresGameRecords::connect(&reader_url).await.unwrap();
    assert_eq!(
        reader
            .detail(&GameId::new("current-ongoing").unwrap())
            .await
            .unwrap()
            .metadata
            .final_move_ply()
            .get(),
        1
    );
    reader.close().await;
    store.close().await.unwrap();
}

#[tokio::test]
async fn current_schema_rejects_missing_metadata_and_inconsistent_finish_time() {
    let f = Fixture::new().await;
    f.completed("game", "black", "white", 1, "white", 20).await;
    for mutation in [
        "rule=NULL",
        "visibility=NULL",
        "final_move_ply=NULL",
        "finished_at=NULL",
        "winner=NULL",
        "reason=NULL",
    ] {
        assert!(
            f.raw
                .execute_unprepared(&format!("UPDATE games SET {mutation} WHERE id='game'"))
                .await
                .is_err(),
            "{mutation}"
        );
    }
}

#[tokio::test]
async fn identifier_encoding_preserves_binary_ties_and_cursor_round_trips() {
    let f = Fixture::new().await;
    let mut names = vec![
        "game\0",
        "game\u{1}",
        "game\u{1}\u{1}",
        "gameA",
        "game\\0",
        "game棋",
    ];
    for name in &names {
        f.completed(name, "black\0\u{1}", "white", 0, "white", 10)
            .await;
    }
    names.sort_by(|a, b| b.as_bytes().cmp(a.as_bytes()));
    let reader = f.reader().await;
    let mut cursor = None;
    let mut actual = Vec::new();
    loop {
        let mut query = filters();
        query.participant = Some(UserId::new("black\0\u{1}").unwrap());
        let page = reader
            .search(&search(query, 1, cursor.as_deref()))
            .await
            .unwrap();
        for item in page.items {
            let detail = reader.detail(item.id()).await.unwrap();
            assert_eq!(detail.metadata.id(), item.id());
            actual.push(item.id().as_str().to_string());
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(actual, names);
}

fn storage_id(value: &str) -> String {
    value
        .replace('\u{1}', "\u{1}\u{2}")
        .replace('\0', "\u{1}\u{1}")
}

#[tokio::test]
async fn current_schema_remains_readable_and_writable_with_query_builders() {
    let database = pg_test_support::TestDatabase::new().await;
    let raw = Database::connect(database.url()).await.unwrap();
    raw.execute_unprepared(include_str!("fixtures/schema-v7.sql"))
        .await
        .unwrap();
    let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
    let games = GameService::new(store.clone());
    let id = WriteId::new("existing-schema").unwrap();
    games
        .create(
            id.clone(),
            Participants::new(UserId::new("black").unwrap(), UserId::new("white").unwrap())
                .unwrap(),
        )
        .await
        .unwrap();
    games
        .resign(
            &id,
            Resign {
                request_id: RequestId::new("end").unwrap(),
                expected_seq: 0,
                user_id: UserId::new("white").unwrap(),
            },
        )
        .await
        .unwrap();
    let reader = PostgresGameRecords::connect(&database.reader_url().await)
        .await
        .unwrap();
    let detail = reader
        .detail(&GameId::new(id.as_str()).unwrap())
        .await
        .unwrap();
    assert_eq!(detail.metadata.final_seq().get(), 1);
    assert_eq!(detail.history[0].action, PublicAction::Resign);
    reader.close().await;
    store.close().await.unwrap();
}

#[tokio::test]
async fn fresh_builder_schema_matches_the_current_schema_fixture() {
    let previous = pg_test_support::TestDatabase::new().await;
    let previous_db = Database::connect(previous.url()).await.unwrap();
    previous_db
        .execute_unprepared(include_str!("fixtures/schema-v7.sql"))
        .await
        .unwrap();
    let current = pg_test_support::TestDatabase::new().await;
    let store = SeaOrmStore::connect(current.url()).await.unwrap();
    let current_db = Database::connect(current.url()).await.unwrap();
    let catalog = "SELECT definition FROM (
        SELECT 'column|' || json_build_array(table_name, ordinal_position, column_name, data_type, is_nullable, column_default, collation_name, is_identity, identity_generation)::text AS definition
        FROM information_schema.columns WHERE table_schema = 'public'
        UNION ALL
        SELECT 'constraint|' || json_build_array(c.relname, k.conname, pg_get_constraintdef(k.oid), k.condeferrable, k.condeferred, k.convalidated)::text
        FROM pg_constraint k JOIN pg_class c ON c.oid = k.conrelid JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public'
        UNION ALL
        SELECT 'index|' || indexdef FROM pg_indexes WHERE schemaname = 'public'
    ) definitions ORDER BY definition";
    let mut definitions = Vec::new();
    for db in [&previous_db, &current_db] {
        definitions.push(
            db.query_all_raw(Statement::from_string(DbBackend::Postgres, catalog))
                .await
                .unwrap()
                .into_iter()
                .map(|row| row.try_get::<String>("", "definition").unwrap())
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(definitions[0], definitions[1]);
    store.close().await.unwrap();
}

#[tokio::test]
async fn detail_keeps_one_database_view_when_history_changes_between_reads() {
    let f = Fixture::new().await;
    f.completed("snapshot", "black", "white", 1, "white", 50)
        .await;
    let reader = f.reader().await;
    let key = GameId::new("snapshot").unwrap();
    let before = reader.detail(&key).await.unwrap();
    let tx = f.raw.begin().await.unwrap();
    tx.execute_unprepared("LOCK TABLE journal IN ACCESS EXCLUSIVE MODE")
        .await
        .unwrap();
    let pending = tokio::spawn(async move { reader.detail(&key).await });
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let row = f.raw.query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT count(*) AS waiting FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE '%journal%' AND pid<>pg_backend_pid()",
            )).await.unwrap().unwrap();
            if row.try_get::<i64>("", "waiting").unwrap() > 0 { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    tx.execute_unprepared("DELETE FROM outbox_events WHERE game_id='snapshot'; DELETE FROM journal WHERE game_id='snapshot' AND seq=2; UPDATE journal SET action=2,usi=NULL,user_id='white',in_check=FALSE,winner=1,reason=2 WHERE game_id='snapshot' AND seq=1; UPDATE games SET version=1,final_move_ply=0 WHERE id='snapshot'").await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(pending.await.unwrap().unwrap(), before);
    let after = f
        .reader()
        .await
        .detail(&GameId::new("snapshot").unwrap())
        .await
        .unwrap();
    assert_eq!(after.metadata.final_seq().get(), 1);
    assert_eq!(after.history.len(), 1);
}

#[tokio::test]
async fn finish_time_is_writer_utc_and_retries_preserve_it() {
    fn micros() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros()
            .try_into()
            .unwrap()
    }
    let f = Fixture::new().await;
    let key = WriteId::new("timed").unwrap();
    f.games
        .create(
            key.clone(),
            Participants::new(UserId::new("black").unwrap(), UserId::new("white").unwrap())
                .unwrap(),
        )
        .await
        .unwrap();
    let command = Resign {
        request_id: RequestId::new("finish").unwrap(),
        expected_seq: 0,
        user_id: UserId::new("white").unwrap(),
    };
    let before = micros();
    f.games.resign(&key, command.clone()).await.unwrap();
    let after = micros();
    let reader = f.reader().await;
    let id = GameId::new("timed").unwrap();
    let result = reader.detail(&id).await.unwrap();
    assert!((before..=after).contains(&result.metadata.finished_at().get()));
    assert_eq!(result.metadata.final_seq().get(), 1);
    assert_eq!(result.metadata.final_move_ply().get(), 0);
    f.games.resign(&key, command).await.unwrap();
    assert_eq!(reader.detail(&id).await.unwrap(), result);
}

#[tokio::test]
async fn public_history_accepts_4096_structural_operations_without_replaying_legality() {
    let f = Fixture::new().await;
    f.raw.execute_unprepared("INSERT INTO games (id,black,white,version,winner,reason,rule,visibility,finished_at,final_move_ply) VALUES ('bounded','black','white',4096,1,2,'standard','public',1,4095); INSERT INTO journal (game_id,seq,request_id,user_id,expected_seq,action,usi,in_check,winner,reason) SELECT 'bounded',n,'request-' || n,CASE WHEN n%2=1 THEN 'black' ELSE 'white' END,n-1,CASE WHEN n=4096 THEN 2 ELSE 1 END,CASE WHEN n=4096 THEN NULL WHEN n%2=1 THEN '7g7f' ELSE '3c3d' END,FALSE,CASE WHEN n=4096 THEN 1 ELSE NULL END,CASE WHEN n=4096 THEN 2 ELSE NULL END FROM generate_series(1,4096) AS n").await.unwrap();
    let reader = f.reader().await;
    let id = GameId::new("bounded").unwrap();
    let detail = reader.detail(&id).await.unwrap();
    assert_eq!(detail.history.len(), 4096);
    assert_eq!(detail.metadata.final_seq().get(), 4096);
    assert_eq!(detail.metadata.final_move_ply().get(), 4095);
    f.raw
        .execute_unprepared(
            "UPDATE games SET version=4097,final_move_ply=4096,winner=2 WHERE id='bounded'",
        )
        .await
        .unwrap();
    assert_eq!(reader.detail(&id).await, Err(Error::HistoryTooLong));
}
