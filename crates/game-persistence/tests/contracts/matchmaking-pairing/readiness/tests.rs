use super::*;
use game_application::ApplicationError;

async fn paired(f: &Fixture) -> (TicketId, TicketId, GameId) {
    let one = f.join("one", POOL).await;
    let two = f.join("two", POOL).await;
    let game = f.store.tick(POOL).await.unwrap().unwrap();
    (one, two, game)
}

async fn no_game_artifacts(f: &Fixture) {
    for table in [
        "games",
        "rated_games",
        "rating_slots",
        "rating_jobs",
        "journal",
        "outbox_events",
    ] {
        assert_eq!(f.count(table).await, 0, "{table}");
    }
}

async fn fixed_clock(f: &Fixture) {
    f.raw
        .execute_unprepared(
            "CREATE TABLE readiness_clock (now_ms BIGINT NOT NULL); \
             INSERT INTO readiness_clock VALUES (2000000000000); \
             ALTER FUNCTION pg_catalog.clock_timestamp() RENAME TO original_clock_timestamp; \
             CREATE FUNCTION pg_catalog.clock_timestamp() RETURNS timestamptz \
             LANGUAGE sql VOLATILE AS $$ \
                 SELECT to_timestamp(now_ms / 1000.0) FROM readiness_clock \
             $$",
        )
        .await
        .unwrap();
    let time: i64 = f
        .raw
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint AS n",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "n")
        .unwrap();
    assert_eq!(time, 2_000_000_000_000);
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/matchmaking-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL と SeaOrmStore を使う。ready とハートビートの再試行、別接続での復旧、対局操作を直接呼ぶ。HTTP や RPC は通らない。", reason: "一人目の ready は永続化され、再試行・ハートビート・取消で期限を変えない。相手の確認前は対局 API が NotFound となり、対局・予約枠・ジョブを作らない。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
#[tokio::test]
async fn first_ready_is_durable_and_the_prospective_game_cannot_be_played() {
    let f = Fixture::new().await;
    let (one, two, game) = paired(&f).await;
    let first = active(&f.store, &uid("one")).await;
    let second = active(&f.store, &uid("two")).await;
    assert_eq!(first.id, one);
    assert_eq!(second.id, two);
    let TicketState::Matched {
        game_id,
        side,
        ready: false,
        ready_deadline,
    } = first.state else {
        panic!("expected an unconfirmed match");
    };
    assert_eq!(game_id, game);
    assert!(matches!(second.state, TicketState::Matched {
        game_id: ref other_game, side: other_side, ready: false, ready_deadline: other_deadline,
    } if other_game == &game && other_side != side && other_deadline == ready_deadline));
    let confirmed = f.store.ready(&uid("one"), one).await.unwrap();
    assert_eq!(confirmed.state, TicketState::Matched {
        game_id: game.clone(), side, ready: true, ready_deadline,
    });
    let recovered = SeaOrmStore::connect(f._db.url()).await.unwrap();
    assert_eq!(recovered.ready(&uid("one"), one).await.unwrap(), confirmed);
    assert_eq!(active(&recovered, &uid("one")).await, confirmed);
    assert_eq!(f.store.cancel(&uid("one"), one).await.unwrap(), confirmed);
    assert_eq!(f.store.heartbeat(&uid("one"), one, HeartbeatSequence::new(1).unwrap()).await.unwrap(), confirmed);
    assert_eq!(active(&f.store, &uid("two")).await, second);
    assert_eq!(f.store.ready(&uid("two"), one).await, Err(MatchmakingError::NotFound));
    assert_eq!(f.store.ready(&uid("one"), TicketId::new()).await, Err(MatchmakingError::NotFound));
    assert_eq!(f.store.join(uid("one"), TicketId::new(), Pool::Byoyomi30).await.unwrap().ticket.state,
        TicketState::Refused(RefusalReason::AlreadyMatched));
    assert_eq!(f.store.load_game(&game).await, Err(ApplicationError::NotFound));
    assert!(matches!(f.store.load_game_for_viewer(&game, &uid("one")).await, Err(ApplicationError::NotFound)));
    let service = GameService::new(f.store.clone());
    assert_eq!(service.resign(&game, Resign {
        request_id: RequestId::new("too-early").unwrap(),
        user_id: uid("one"), expected_seq: 0,
    }).await, Err(ApplicationError::NotFound));
    no_game_artifacts(&f).await;
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/matchmaking-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL と SeaOrmStore を使い、両者の ready を同時実行する。HTTP や RPC は通らない。", reason: "待機中の ready は同意を先取りしない。同時 ready は予定 UUID、先後、プール、固定レーティングを持つ対局一件と予約枠二件だけを作り、再送は同じ Playing を返す。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
#[tokio::test]
async fn concurrent_confirmations_start_once_without_advance_consent() {
    let f = Fixture::new().await;
    let one = f.join("one", POOL).await;
    let waiting = active(&f.store, &uid("one")).await;
    assert_eq!(waiting.id, one);
    assert_eq!(f.store.ready(&uid("one"), one).await.unwrap(), waiting);
    let two = f.join("two", POOL).await;
    let game = f.store.tick(POOL).await.unwrap().unwrap();
    let before = active(&f.store, &uid("one")).await;
    assert_eq!(before.id, one);
    assert!(matches!(before.state, TicketState::Matched { ready: false, .. }));
    let owner_one = uid("one");
    let owner_two = uid("two");
    let (a, b) = tokio::join!(f.store.ready(&owner_one, one), f.store.ready(&owner_two, two));
    assert!(a.is_ok());
    assert!(b.is_ok());
    let identity = pg_test_support::rated_identity(f._db.url(), &game).await.unwrap();
    assert_eq!(identity.pool, POOL);
    assert_eq!(identity.black_start, RatingState::Unrated);
    assert_eq!(identity.white_start, RatingState::Unrated);
    let recovered = SeaOrmStore::connect(f._db.url()).await.unwrap();
    for (user, ticket) in [(&owner_one, one), (&owner_two, two)] {
        let current = recovered.ready(user, ticket).await.unwrap();
        assert_eq!(current.id, ticket);
        assert_eq!(current.state, TicketState::Playing {
            game_id: game.clone(), side: identity.participants.side(user).unwrap(),
        });
        assert_eq!(recovered.ready(user, ticket).await.unwrap(), current);
        assert_eq!(recovered.current(user).await.unwrap(), Participation::Active(current));
    }
    assert_eq!(f.count("games").await, 1);
    assert_eq!(f.count("rated_games").await, 1);
    assert_eq!(f.count("rating_slots").await, 2);
    assert_eq!(f.count("rating_jobs").await, 0);
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/matchmaking-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL と SeaOrmStore を使い、片側のみ ready にした後、共有期限を強制的に過去にする。六種類のリポジトリー入口を個別に実行する。", reason: "join・current・heartbeat・cancel・ready・空プールの tick の各入口が両者を Voided にする。対局・予約枠・ジョブを作らず、両者の再参加を許し、古い再送は新しい待機を変更しない。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
#[tokio::test]
async fn every_entry_point_expires_the_pair_and_stale_retries_leave_new_tickets_alone() {
    for operation in ["join", "current", "heartbeat", "cancel", "ready", "tick"] {
        let f = Fixture::new().await;
        let (one, two, game) = paired(&f).await;
        let confirmed = f.store.ready(&uid("one"), one).await.unwrap();
        let tracking = HeartbeatSequence::new(1).unwrap();
        assert_eq!(f.store.heartbeat(&uid("one"), one, tracking).await.unwrap(), confirmed);
        f.raw.execute_unprepared("UPDATE matchmaking_tickets SET ready_deadline=0 WHERE state='matched'").await.unwrap();
        let next = TicketId::new();
        match operation {
            "join" => { assert!(f.store.join(uid("one"), next, POOL).await.unwrap().created); }
            "current" => { assert_eq!(f.store.current(&uid("one")).await.unwrap(), Participation::Idle); }
            "heartbeat" => { f.store.heartbeat(&uid("one"), one, tracking).await.unwrap(); }
            "cancel" => { f.store.cancel(&uid("one"), one).await.unwrap(); }
            "ready" => { f.store.ready(&uid("one"), one).await.unwrap(); }
            "tick" => { assert_eq!(f.store.tick(Pool::Byoyomi30).await.unwrap(), None); }
            _ => unreachable!(),
        }
        let voided_count: i64 = f.raw.query_one_raw(Statement::from_string(DbBackend::Postgres,
            "SELECT count(*) AS n FROM matchmaking_tickets WHERE state='voided'"))
            .await.unwrap().unwrap().try_get("", "n").unwrap();
        assert_eq!(voided_count, 2, "{operation}");
        no_game_artifacts(&f).await;
        if operation != "join" {
            assert!(f.store.join(uid("one"), next, POOL).await.unwrap().created);
        }
        let other_next = f.join("two", Pool::Byoyomi30).await;
        let waiting_one = active(&f.store, &uid("one")).await;
        let waiting_two = active(&f.store, &uid("two")).await;
        assert_eq!(waiting_one.id, next);
        assert_eq!(waiting_two.id, other_next);
        let recovered = SeaOrmStore::connect(f._db.url()).await.unwrap();
        for (user, old) in [("one", one), ("two", two)] {
            let stored = recovered.heartbeat(&uid(user), old, tracking).await.unwrap();
            assert!(matches!(stored.state, TicketState::Voided { game_id: ref id, .. } if id == &game));
            assert_eq!(f.store.ready(&uid(user), old).await.unwrap(), stored);
            assert_eq!(f.store.cancel(&uid(user), old).await.unwrap(), stored);
            assert_eq!(f.store.join(uid(user), old, POOL).await.unwrap().ticket, stored);
        }
        assert!(matches!(waiting_one.state, TicketState::Waiting(_)));
        assert!(matches!(waiting_two.state, TicketState::Waiting(_)));
        assert_eq!(active(&f.store, &uid("one")).await, waiting_one);
        assert_eq!(active(&f.store, &uid("two")).await, waiting_two);
    }
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/matchmaking-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の独立したテスト用 PostgreSQL データベース内の時計関数を置換し、期限の一ミリ秒前と等号時を固定する。HTTP や RPC は通らない。", reason: "マッチ時刻から十五秒の期限は一ミリ秒前まで開始を許し、等号時は ready・再参加・tick の競合でも両者を Voided にして対局を作らない。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
#[tokio::test]
async fn the_database_deadline_is_exclusive_including_concurrent_rejoin() {
    for elapsed in [14_999, 15_000] {
        let f = Fixture::new().await;
        fixed_clock(&f).await;
        let (one, two, game) = paired(&f).await;
        let matched = active(&f.store, &uid("one")).await;
        assert_eq!(matched.id, one);
        assert!(matches!(matched.state, TicketState::Matched { ready_deadline: 2_000_000_015_000, .. }));
        f.store.ready(&uid("one"), one).await.unwrap();
        f.raw.execute_unprepared(&format!("UPDATE readiness_clock SET now_ms=2000000000000+{elapsed}")).await.unwrap();
        if elapsed == 14_999 {
            assert!(matches!(f.store.ready(&uid("two"), two).await.unwrap().state,
                TicketState::Playing { game_id, .. } if game_id == game));
            assert_eq!(f.count("games").await, 1);
        } else {
            let owner = uid("two");
            let next = TicketId::new();
            let (ready, joined, tick) = tokio::join!(
                f.store.ready(&owner, two),
                f.store.join(uid("one"), next, Pool::Byoyomi30),
                f.store.tick(POOL),
            );
            assert!(matches!(ready.unwrap().state, TicketState::Voided { .. }));
            assert!(joined.unwrap().created);
            assert_eq!(tick.unwrap(), None);
            assert!(matches!(f.store.heartbeat(&uid("one"), one, HeartbeatSequence::new(1).unwrap()).await.unwrap().state, TicketState::Voided { .. }));
            no_game_artifacts(&f).await;
        }
    }
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/matchmaking-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の独立したテスト用 PostgreSQL データベース内の時計関数を置換する。レーティング行ロックを保持し、二人目の ready が待つことを pg_stat_activity で確認してから DB 時計を期限まで進める。", reason: "ready はレーティングロック取得後のデータベース時刻でも期限を検査し、両者を Voided にする。対局・予約枠・ジョブは作らない。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
#[tokio::test]
async fn a_rating_lock_wait_cannot_extend_the_ready_deadline() {
    let f = Fixture::new().await;
    fixed_clock(&f).await;
    let (one, two, _) = paired(&f).await;
    f.store.ready(&uid("one"), one).await.unwrap();
    let held = f.raw.begin().await.unwrap();
    held.execute_unprepared("SELECT * FROM rating_states ORDER BY user_id FOR UPDATE").await.unwrap();
    let store = f.store.clone();
    let pending = tokio::spawn(async move { store.ready(&uid("two"), two).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let blocked: i64 = f.raw.query_one_raw(Statement::from_string(DbBackend::Postgres,
                "SELECT count(*) AS n FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE '%rating_states%'"))
                .await.unwrap().unwrap().try_get("", "n").unwrap();
            if blocked > 0 { break; }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }).await.unwrap();
    f.raw.execute_unprepared("UPDATE readiness_clock SET now_ms=2000000015000").await.unwrap();
    held.commit().await.unwrap();
    assert!(matches!(pending.await.unwrap().unwrap().state, TicketState::Voided { .. }));
    assert!(matches!(f.store.heartbeat(&uid("one"), one, HeartbeatSequence::new(1).unwrap()).await.unwrap().state, TicketState::Voided { .. }));
    no_game_artifacts(&f).await;
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/matchmaking-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL と SeaOrmStore を使い、Playing への更新に SQL 失敗を注入する。HTTP や RPC は通らない。", reason: "二人目の ready と対局・予約枠の作成をまとめて取り消し、一人目の確認だけを維持する。失敗除去後の同じ ready は予定 UUID の対局一件だけを作る。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
#[tokio::test]
async fn failure_after_game_creation_rolls_back_confirmation_and_all_game_artifacts() {
    let f = Fixture::new().await;
    let (one, two, game) = paired(&f).await;
    let confirmed = f.store.ready(&uid("one"), one).await.unwrap();
    let unconfirmed = active(&f.store, &uid("two")).await;
    assert_eq!(unconfirmed.id, two);
    f.raw.execute_unprepared("CREATE FUNCTION reject_start() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.state='playing' THEN RAISE EXCEPTION 'injected start failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_start BEFORE UPDATE ON matchmaking_tickets FOR EACH ROW EXECUTE FUNCTION reject_start()").await.unwrap();
    assert_eq!(f.store.ready(&uid("two"), two).await, Err(MatchmakingError::Unavailable));
    assert_eq!(active(&f.store, &uid("one")).await, confirmed);
    assert_eq!(active(&f.store, &uid("two")).await, unconfirmed);
    no_game_artifacts(&f).await;
    f.raw.execute_unprepared("DROP TRIGGER reject_start ON matchmaking_tickets").await.unwrap();
    let started = f.store.ready(&uid("two"), two).await.unwrap();
    assert!(matches!(started.state, TicketState::Playing { ref game_id, .. } if game_id == &game));
    assert_eq!(f.store.ready(&uid("two"), two).await.unwrap(), started);
    assert_eq!(f.count("games").await, 1);
    assert_eq!(f.count("rating_slots").await, 2);
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/matchmaking-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL のチケットを直接変更し、片側欠落・プール不一致・期限不一致を個別に作る。HTTP や RPC は通らない。", reason: "不完全な組は ready と期限切れ処理の両方で Corrupt となり、対局・予約枠・ジョブを作らない。SQL 制約も側の重複と予定情報の欠落を拒否する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
#[tokio::test]
async fn inconsistent_pairs_are_rejected_before_start_or_expiry() {
    for corruption in ["missing", "pool", "deadline"] {
        let f = Fixture::new().await;
        let (one, _, _) = paired(&f).await;
        assert!(f.raw.execute_unprepared("UPDATE matchmaking_tickets SET matched_side=1").await.is_err());
        assert!(f.raw.execute_unprepared("UPDATE matchmaking_tickets SET prospective_game_id=NULL").await.is_err());
        let sql = match corruption {
            "missing" => "DELETE FROM matchmaking_tickets WHERE user_id='two'",
            "pool" => "UPDATE matchmaking_tickets SET pool='byoyomi_30' WHERE user_id='two'",
            "deadline" => "UPDATE matchmaking_tickets SET ready_deadline=ready_deadline+1 WHERE user_id='two'",
            _ => unreachable!(),
        };
        f.raw.execute_unprepared(sql).await.unwrap();
        assert_eq!(f.store.ready(&uid("one"), one).await, Err(MatchmakingError::Corrupt));
        f.raw.execute_unprepared("UPDATE matchmaking_tickets SET ready_deadline=0 WHERE user_id='one'").await.unwrap();
        assert_eq!(f.store.current(&uid("one")).await, Err(MatchmakingError::Corrupt));
        no_game_artifacts(&f).await;
    }
}
}
