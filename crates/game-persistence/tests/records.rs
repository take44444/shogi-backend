use game_application::{
    GameId as WriteId, GameRepository, GameService, MakeMove, Participants, RequestId, Resign,
    UserId,
};
use game_persistence::{SeaOrmStore, SqliteGameRecords};
use game_record_application::{
    Error, Filters, FinishReason, GameId, MAX_HISTORY, PublicAction, RecordRepository, Rule, Search,
};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use shogi_domain::Player;
use std::sync::Arc;

struct Fixture {
    _directory: tempfile::TempDir,
    path: std::path::PathBuf,
    games: GameService,
    raw: DatabaseConnection,
}
impl Fixture {
    async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("game.db");
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
        let games = GameService::new(store);
        let raw = Database::connect(&url).await.unwrap();
        Self {
            _directory: directory,
            path,
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
                Participants {
                    black: UserId::new(black).unwrap(),
                    white: UserId::new(white).unwrap(),
                },
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
                DbBackend::Sqlite,
                "UPDATE games SET finished_at = ? WHERE id = ?",
                [time.into(), id.as_str().into()],
            ))
            .await
            .unwrap();
    }
    async fn reader(&self) -> SqliteGameRecords {
        SqliteGameRecords::open(&self.path).await.unwrap()
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
    page.items.iter().map(|i| i.id.as_str()).collect()
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
            Participants {
                black: UserId::new("alice").unwrap(),
                white: UserId::new("bob").unwrap(),
            },
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
    assert_eq!(detail.metadata.final_move_ply, 2);
    assert_eq!(detail.metadata.final_seq, 3);
    assert_eq!(
        detail.history.iter().map(|o| o.seq).collect::<Vec<_>>(),
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
            Participants {
                black: UserId::new("black").unwrap(),
                white: UserId::new("white").unwrap(),
            },
        )
        .await
        .unwrap();
    f.raw.execute_unprepared("CREATE TRIGGER fail_record BEFORE INSERT ON journal BEGIN SELECT RAISE(ABORT,'failure'); END").await.unwrap();
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
            DbBackend::Sqlite,
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
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("absent.db");
    assert!(SqliteGameRecords::open(&missing).await.is_err());
    assert!(!missing.exists());
    for sql in [
        "UPDATE schema_version SET version=3",
        "UPDATE schema_version SET version=99",
        "DELETE FROM schema_version",
        "DROP TABLE journal",
        "ALTER TABLE games RENAME COLUMN finished_at TO missing",
    ] {
        let f = Fixture::new().await;
        f.raw.execute_unprepared(sql).await.unwrap();
        assert!(
            matches!(SqliteGameRecords::open(&f.path).await, Err(Error::Corrupt)),
            "{sql}"
        );
    }
}

async fn current_rows(db: &DatabaseConnection) -> Vec<Vec<String>> {
    let mut state = Vec::new();
    for sql in [
        "SELECT json_array(id,black,white,version,winner,reason,rule,visibility,finished_at,final_move_ply) AS value FROM games WHERE id LIKE 'current-%' ORDER BY id",
        "SELECT json_array(game_id,seq,request_id,user_id,expected_seq,action,usi,in_check,winner,reason) AS value FROM journal WHERE game_id LIKE 'current-%' ORDER BY game_id,seq",
        "SELECT json_array(id,game_id,seq,event_kind,encoding_version,request_id,user_id,usi,in_check,winner,reason,published) AS value FROM outbox_events WHERE game_id LIKE 'current-%' ORDER BY id",
    ] {
        state.push(
            db.query_all_raw(Statement::from_string(DbBackend::Sqlite, sql))
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
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("records.db");
    let url = format!("sqlite://{}?mode=rwc", path.display());
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
            SELECT id,1,'move','black',0,1,'7g7f',0,NULL,NULL FROM games;
        INSERT INTO journal (game_id,seq,request_id,user_id,expected_seq,action,usi,in_check,winner,reason)
            SELECT id,2,'end','white',1,2,NULL,0,1,2 FROM games WHERE version=2;
        INSERT INTO outbox_events (id,game_id,seq,event_kind,encoding_version,request_id,user_id,usi,in_check,winner,reason,published)
            SELECT row_number() OVER (ORDER BY game_id,seq)+100,game_id,seq,CASE action WHEN 1 THEN 0 ELSE 1 END,1,request_id,user_id,usi,CASE action WHEN 1 THEN in_check ELSE NULL END,winner,reason,CASE action WHEN 1 THEN 1 ELSE 0 END FROM journal;").await.unwrap();
    let before = current_rows(&db).await;
    for _ in 0..2 {
        let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
        assert_eq!(current_rows(&db).await, before);
        assert!(
            db.query_all_raw(Statement::from_string(
                DbBackend::Sqlite,
                "PRAGMA foreign_key_check"
            ))
            .await
            .unwrap()
            .is_empty()
        );
        let reader = SqliteGameRecords::open(&path).await.unwrap();
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
    let reader = SqliteGameRecords::open(&path).await.unwrap();
    assert_eq!(
        reader
            .detail(&GameId::new("current-ongoing").unwrap())
            .await
            .unwrap()
            .metadata
            .final_move_ply,
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
