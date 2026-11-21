use game_application::{
    ApplicationError, ClockState, GameId, GameRepository, GameService, JournalEntry, MakeMove,
    Participants, RequestId, Resign, UserId, rating::Pool,
};
use game_application::{EventOrigin, OutboxRepository};
use game_persistence::{PostgresOutbox, SeaOrmStore};
use game_record_application::{PublicAction, RecordRepository};
use game_record_persistence::PostgresGameRecords;
use pg_test_support::TestDatabase;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use shogi_domain::{CommandError, FinishReason, Player};
use std::sync::Arc;

fn user(id: &str) -> UserId {
    UserId::new(id).unwrap()
}
fn game(id: &str) -> GameId {
    GameId::new(id).unwrap()
}
fn players(id: &str) -> Participants {
    Participants::new(user(&format!("{id}-black")), user(&format!("{id}-white"))).unwrap()
}
fn movement(id: &str) -> MakeMove {
    MakeMove {
        request_id: RequestId::new("first").unwrap(),
        user_id: players(id).black().clone(),
        expected_seq: 0,
        movement: "7g7f".parse().unwrap(),
    }
}
async fn age(url: &str, id: &GameId, amount: i64) {
    let db = Database::connect(url).await.unwrap();
    for query in [
        "UPDATE games SET created_at=created_at-$2,clock_started=clock_started-$2,clock_deadline=clock_deadline-$2 WHERE id=$1",
        "UPDATE journal SET evaluated_at=evaluated_at-$2,clock_started=clock_started-$2,clock_deadline=clock_deadline-$2 WHERE game_id=$1",
    ] {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            query,
            [id.as_str().into(), amount.into()],
        ))
        .await
        .unwrap();
    }
    db.close().await.unwrap();
}
async fn scalar(url: &str, sql: &str) -> i64 {
    let db = Database::connect(url).await.unwrap();
    let value = db
        .query_one_raw(Statement::from_string(DbBackend::Postgres, sql))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "value")
        .unwrap();
    db.close().await.unwrap();
    value
}

#[traceability::targets(
    game_application::GameService::poll_timeouts,
    game_persistence::SeaOrmStore::begin_game_update,
    game_persistence::SeaOrmStore::expired_games
)]
#[tokio::test]
async fn both_pools_expire_without_actors_and_finish_once_across_services_and_restart() {
    for pool in Pool::ALL {
        let database = TestDatabase::new().await;
        let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
        let id = game("idle");
        let p = players("idle");
        pg_test_support::seed_rated(database.url(), &id, &p, pool)
            .await
            .unwrap();
        let before = store.load_game(&id).await.unwrap();
        assert_eq!(
            before.time_control.initial_main_ms.get(),
            if pool == Pool::TenMinute30 {
                600_000
            } else {
                0
            }
        );
        assert_eq!(before.clock.saved_at(), before.initial_clock.saved_at());
        age(
            database.url(),
            &id,
            if pool == Pool::TenMinute30 {
                631_000
            } else {
                31_000
            },
        )
        .await;
        let a = GameService::new(store.clone());
        let b = GameService::new(Arc::new(
            SeaOrmStore::connect(database.url()).await.unwrap(),
        ));
        let (one, two) = tokio::join!(a.poll_timeouts(16), b.poll_timeouts(16));
        assert_eq!(one.unwrap() + two.unwrap(), 1);
        assert_eq!(a.poll_timeouts(16).await.unwrap(), 0);
        let terminal = store.load_game(&id).await.unwrap();
        assert_eq!(terminal.version, 1);
        assert_eq!(terminal.result.unwrap().winner, Player::White);
        assert_eq!(terminal.result.unwrap().reason, FinishReason::Timeout);
        match terminal.journal.as_slice() {
            [
                JournalEntry::Timeout {
                    loser,
                    deadline_at,
                    evaluated_at,
                    acceptance,
                    ..
                },
            ] => {
                assert_eq!(*loser, Player::Black);
                assert!(*evaluated_at >= *deadline_at);
                assert_eq!(acceptance.clock.saved_at(), *deadline_at);
            }
            other => panic!("expected system timeout: {other:?}"),
        }
        assert!(
            matches!(terminal.clock,ClockState::Stopped { main_remaining_ms,.. } if main_remaining_ms[0].get()==0)
        );
        let restarted = GameService::new(Arc::new(
            SeaOrmStore::connect(database.url()).await.unwrap(),
        ));
        let snapshot = restarted.snapshot(&id).await.unwrap();
        assert_eq!(snapshot.clock, terminal.clock);
        assert!(snapshot.legal_moves.is_empty());
        assert_eq!(
            restarted
                .make_move(
                    &id,
                    MakeMove {
                        expected_seq: 1,
                        ..movement("idle")
                    }
                )
                .await,
            Err(ApplicationError::Domain(CommandError::GameFinished))
        );
        assert_eq!(
            scalar(database.url(), "SELECT count(*) AS value FROM journal").await,
            1
        );
        assert_eq!(
            scalar(
                database.url(),
                "SELECT count(*) AS value FROM outbox_events"
            )
            .await,
            1
        );
        assert_eq!(
            scalar(database.url(), "SELECT count(*) AS value FROM rating_jobs").await,
            1
        );
        assert_eq!(
            scalar(
                database.url(),
                "SELECT count(*) AS value FROM matchmaking_tickets WHERE state='finished'"
            )
            .await,
            2
        );
        let mut outbox = PostgresOutbox::connect(database.url()).await.unwrap();
        assert_eq!(
            outbox.next_pending().await.unwrap().unwrap().event.origin,
            EventOrigin::System
        );
        let reader = PostgresGameRecords::connect(&database.reader_url().await)
            .await
            .unwrap();
        let detail = reader
            .detail(&game_record_application::GameId::new(id.as_str()).unwrap())
            .await
            .unwrap();
        assert_eq!(detail.history[0].action, PublicAction::Timeout);
        assert_eq!(detail.history[0].player, Player::Black);
        assert_eq!(detail.metadata.final_move_ply().get(), 0);
        let finished_at = detail.metadata.finished_at().get();
        let search = game_record_application::Search::new(
            game_record_application::Filters {
                participant: None,
                rule: None,
                winner: None,
                reason: Some(FinishReason::Timeout),
                from: finished_at - 1,
                to: finished_at + 1,
                min_ply: None,
                max_ply: None,
            },
            30,
            None,
        )
        .unwrap();
        assert_eq!(reader.search(&search).await.unwrap().items.len(), 1);
        reader.close().await;
        let jobs = Arc::new(
            game_persistence::SeaOrmRatingJobs::connect(database.url())
                .await
                .unwrap(),
        );
        let policy = game_application::rating::WorkerPolicy::new(
            std::time::Duration::from_secs(5),
            std::num::NonZeroU32::new(3).unwrap(),
            std::time::Duration::ZERO,
        )
        .unwrap();
        let worker = game_application::rating::RatingWorker::new(jobs, policy);
        let owner = game_application::rating::WorkerId::new("clock-test").unwrap();
        assert_eq!(
            worker.tick(&owner).await.unwrap(),
            game_application::rating::TickOutcome::Applied
        );
        assert_eq!(
            worker.tick(&owner).await.unwrap(),
            game_application::rating::TickOutcome::Idle
        );
        assert_eq!(
            scalar(
                database.url(),
                "SELECT count(*) AS value FROM rating_applications"
            )
            .await,
            1
        );
        assert_eq!(
            scalar(
                database.url(),
                "SELECT sum(completed)::bigint AS value FROM rating_states"
            )
            .await,
            2
        );
        assert_eq!(
            scalar(
                database.url(),
                "SELECT rating AS value FROM rating_states WHERE user_id='idle-black'"
            )
            .await,
            1484
        );
        assert_eq!(
            scalar(
                database.url(),
                "SELECT rating AS value FROM rating_states WHERE user_id='idle-white'"
            )
            .await,
            1516
        );
        store.close().await.unwrap();
    }
}

#[traceability::targets(
    game_application::GameService::make_move,
    game_application::GameService::resign
)]
#[tokio::test]
async fn expired_commands_lose_to_timeout_while_original_acceptance_replays_after_finish() {
    let database = TestDatabase::new().await;
    let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
    let id = game("active");
    let p = players("active");
    pg_test_support::seed_rated(database.url(), &id, &p, Pool::Byoyomi30)
        .await
        .unwrap();
    let service = GameService::new(store.clone());
    let request = movement("active");
    let accepted = service.make_move(&id, request.clone()).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(30_050)).await;
    let retained = store.load_game(&id).await.unwrap().journal[0].acceptance();
    assert_eq!(accepted, retained);
    let other = GameService::new(Arc::new(
        SeaOrmStore::connect(database.url()).await.unwrap(),
    ));
    let resign = Resign {
        request_id: RequestId::new("late").unwrap(),
        user_id: p.white().clone(),
        expected_seq: 0,
    };
    let (a, b) = tokio::join!(
        service.resign(&id, resign.clone()),
        other.make_move(
            &id,
            MakeMove {
                request_id: RequestId::new("late-move").unwrap(),
                user_id: p.white().clone(),
                expected_seq: 1,
                movement: "3c3d".parse().unwrap()
            }
        )
    );
    assert_eq!(a, Err(ApplicationError::Domain(CommandError::GameFinished)));
    assert!(matches!(
        b,
        Err(ApplicationError::Domain(CommandError::GameFinished))
            | Err(ApplicationError::SequenceMismatch { .. })
    ));
    assert_eq!(service.make_move(&id, request).await.unwrap(), retained);
    let terminal = service.snapshot(&id).await.unwrap();
    assert_eq!(terminal.seq, 2);
    assert_eq!(terminal.result.unwrap().winner, Player::Black);
    assert_eq!(terminal.result.unwrap().reason, FinishReason::Timeout);
    assert_eq!(
        store
            .load_game(&id)
            .await
            .unwrap()
            .journal
            .iter()
            .filter_map(JournalEntry::request)
            .count(),
        1
    );
    assert_eq!(accepted.seq, retained.seq);
}

#[traceability::targets(game_application::GameService::poll_timeouts)]
#[tokio::test]
async fn corrupt_first_candidate_does_not_prevent_healthy_candidate_from_finishing() {
    let database = TestDatabase::new().await;
    let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
    for id in ["broken", "healthy"] {
        pg_test_support::seed_rated(database.url(), &game(id), &players(id), Pool::Byoyomi30)
            .await
            .unwrap();
        age(
            database.url(),
            &game(id),
            if id == "broken" { 32_000 } else { 31_000 },
        )
        .await;
    }
    let db = Database::connect(database.url()).await.unwrap();
    db.execute_unprepared("UPDATE games SET version=1 WHERE id='broken'")
        .await
        .unwrap();
    assert_eq!(
        GameService::new(store.clone()).poll_timeouts(16).await,
        Err(ApplicationError::Corrupt)
    );
    assert_eq!(
        store
            .load_game(&game("healthy"))
            .await
            .unwrap()
            .result
            .unwrap()
            .reason,
        FinishReason::Timeout
    );
    assert_eq!(
        scalar(
            database.url(),
            "SELECT count(*) AS value FROM journal WHERE game_id='healthy'"
        )
        .await,
        1
    );
    db.close().await.unwrap();
    store.close().await.unwrap();
}

#[traceability::targets(game_persistence::SeaOrmStore::begin_game_update)]
#[tokio::test]
async fn terminal_decision_releases_game_lock_and_rechecks_after_admission_lock_wait() {
    use sea_orm::TransactionTrait;
    let database = TestDatabase::new().await;
    let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
    let id = game("lock-order");
    let p = players("lock-order");
    pg_test_support::seed_rated(database.url(), &id, &p, Pool::TenMinute30)
        .await
        .unwrap();
    let raw = Database::connect(database.url()).await.unwrap();
    let admission = raw.begin().await.unwrap();
    admission
        .execute_unprepared("SELECT pg_advisory_xact_lock(6001133723959083849)")
        .await
        .unwrap();
    let service = GameService::new(store.clone());
    let other = service.clone();
    let key = id.clone();
    let white = p.white().clone();
    let pending = tokio::spawn(async move {
        other
            .resign(
                &key,
                Resign {
                    request_id: RequestId::new("resign").unwrap(),
                    user_id: white,
                    expected_seq: 0,
                },
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(3),async {
        loop {
            let row=raw.query_one_raw(Statement::from_string(DbBackend::Postgres,"SELECT count(*) AS value FROM pg_stat_activity WHERE datname=current_database() AND wait_event='advisory' AND pid<>pg_backend_pid()")).await.unwrap().unwrap();
            if row.try_get::<i64>("","value").unwrap()>0 {break;}
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    let independent = GameService::new(Arc::new(
        SeaOrmStore::connect(database.url()).await.unwrap(),
    ));
    let moved = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        independent.make_move(&id, movement("lock-order")),
    )
    .await
    .expect("normal move must acquire the released game row without admission lock")
    .unwrap();
    assert_eq!(moved.seq, 1);
    admission.rollback().await.unwrap();
    assert_eq!(
        pending.await.unwrap(),
        Err(ApplicationError::SequenceMismatch {
            expected: 0,
            actual: 1
        })
    );
    assert!(store.load_game(&id).await.unwrap().result.is_none());
    raw.close().await.unwrap();
    store.close().await.unwrap();
}

#[traceability::targets(game_persistence::SeaOrmStore::begin_game_update)]
#[tokio::test]
async fn time_waiting_for_the_game_row_counts_before_a_move_is_evaluated() {
    use sea_orm::TransactionTrait;
    let database = TestDatabase::new().await;
    let store = Arc::new(SeaOrmStore::connect(database.url()).await.unwrap());
    let id = game("blocked");
    let p = players("blocked");
    pg_test_support::seed_rated(database.url(), &id, &p, Pool::TenMinute30)
        .await
        .unwrap();
    age(database.url(), &id, 629_900).await;
    let raw = Database::connect(database.url()).await.unwrap();
    let tx = raw.begin().await.unwrap();
    tx.execute_unprepared("SELECT id FROM games WHERE id='blocked' FOR UPDATE")
        .await
        .unwrap();
    let service = GameService::new(store.clone());
    let key = id.clone();
    let pending = tokio::spawn(async move { service.make_move(&key, movement("blocked")).await });
    tokio::time::timeout(std::time::Duration::from_secs(3),async {
        loop {
            let row=tx.query_one_raw(Statement::from_string(DbBackend::Postgres,"SELECT clock_deadline <= floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint AS expired FROM games WHERE id='blocked'")).await.unwrap().unwrap();
            if row.try_get::<bool>("","expired").unwrap() {break;}
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(
        pending.await.unwrap(),
        Err(ApplicationError::Domain(CommandError::GameFinished))
    );
    assert!(matches!(
        store.load_game(&id).await.unwrap().journal[0],
        JournalEntry::Timeout { .. }
    ));
    raw.close().await.unwrap();
    store.close().await.unwrap();
}
