use super::*;

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。",
        reason: "同時実行した tick と両者の ready によって、対局一件、rated_game 一件、参加枠二件だけが作られ、一人の現在参加状態が Playing になる。このテストでは参加者や先後の割り当てを比較しない。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "admission", scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。参加受け入れと組合せ処理を並行実行する。", relationship: crate::trace_support::Relationship::Asserts, reason: "異なるプールへの同時参加で、参加受け入れがちょうど一件だけ作られる。" }],
    };
#[tokio::test]
async fn cross_pool_joins_and_multiple_matchers_have_one_durable_winner() {
    let f = Fixture::new().await;
    let (a, b) = tokio::join!(
        f.store.join(uid("one"), TicketId::new(), POOL),
        f.store.join(uid("one"), TicketId::new(), Pool::Byoyomi30)
    );
    let results = [a.unwrap(), b.unwrap()];
    assert_eq!(results.iter().filter(|r| r.created).count(), 1);
    let pool = results
        .iter()
        .find(|r| r.created)
        .unwrap()
        .ticket
        .pool
        .unwrap();
    f.join("two", pool).await;
    let (a, b) = tokio::join!(f.store.tick(pool), f.store.tick(pool));
    assert_eq!(
        [a.unwrap(), b.unwrap()]
            .iter()
            .filter(|r| r.is_some())
            .count(),
        1
    );
    assert_eq!(f.count("games").await, 0);
    f.ready("one").await;
    f.ready("two").await;
    assert_eq!(f.count("games").await, 1);
    assert_eq!(f.count("rated_games").await, 1);
    assert_eq!(f.count("rating_slots").await, 2);
    assert!(
        matches!(f.store.current(&uid("one")).await.unwrap(),Participation::Active(t) if matches!(t.state,TicketState::Playing {..}))
    );
    let refusal = f
        .store
        .join(
            uid("one"),
            TicketId::new(),
            if pool == POOL { Pool::Byoyomi30 } else { POOL },
        )
        .await
        .unwrap();
    assert_eq!(
        refusal.ticket.state,
        TicketState::Refused(RefusalReason::AlreadyPlaying)
    );
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。",
        reason: "参加枠の作成失敗を注入して二人目が ready すると、対局・レーティング対象対局・参加枠の行は 0 件となり、両者は matched を続ける。成功した再試行では、元の Unrated 状態を固定する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "matchmaking-tickets", contract: "admission", scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。SQL の失敗を注入し、終局後のレーティング予約枠を確認する。", relationship: crate::trace_support::Relationship::Asserts, reason: "対局終了後、同じプールへの参加は rating_pending となり、別のプールは引き続き利用できる。" }],
    };
#[tokio::test]
async fn pair_failure_rolls_back_and_terminal_failure_preserves_both_active_tickets() {
    let f = Fixture::new().await;
    let one = f.join("one", POOL).await;
    let two = f.join("two", POOL).await;
    let game = f.store.tick(POOL).await.unwrap().unwrap();
    f.store.ready(&uid("one"), one).await.unwrap();
    f.raw.execute_unprepared("CREATE FUNCTION reject_slot() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected slot failure'; END $$; CREATE TRIGGER reject_slot BEFORE INSERT ON rating_slots FOR EACH ROW EXECUTE FUNCTION reject_slot()").await.unwrap();
    assert_eq!(
        f.store.ready(&uid("two"), two).await,
        Err(MatchmakingError::Unavailable)
    );
    for table in ["games", "rated_games", "rating_slots"] {
        assert_eq!(f.count(table).await, 0);
    }
    for (user, id) in [("one", one), ("two", two)] {
        assert!(matches!(
            f.store.ticket(&uid(user), id).await.unwrap().state,
            TicketState::Matched { .. }
        ));
    }
    f.raw
        .execute_unprepared("DROP TRIGGER reject_slot ON rating_slots")
        .await
        .unwrap();
    f.store.ready(&uid("two"), two).await.unwrap();
    let identity = pg_test_support::rated_identity(f._db.url(), &game)
        .await
        .unwrap();
    assert_eq!(identity.black_start, RatingState::Unrated);
    assert_eq!(identity.white_start, RatingState::Unrated);
    f.raw.execute_unprepared("CREATE TRIGGER reject_job BEFORE INSERT ON rating_jobs FOR EACH ROW EXECUTE FUNCTION reject_slot()").await.unwrap();
    let service = GameService::new(f.store.clone());
    let command = Resign {
        request_id: RequestId::new("finish").unwrap(),
        user_id: uid("one"),
        expected_seq: 0,
    };
    assert!(service.resign(&game, command.clone()).await.is_err());
    assert_eq!(f.store.version(&game).await.unwrap(), 0);
    for table in ["journal", "rating_jobs", "outbox_events"] {
        assert_eq!(f.count(table).await, 0);
    }
    for (user, id) in [("one", one), ("two", two)] {
        assert!(matches!(
            f.store.ticket(&uid(user), id).await.unwrap().state,
            TicketState::Playing { .. }
        ));
    }
    f.raw
        .execute_unprepared("DROP TRIGGER reject_job ON rating_jobs")
        .await
        .unwrap();
    let result = service.resign(&game, command.clone()).await.unwrap();
    assert_eq!(service.resign(&game, command).await.unwrap(), result);
    assert_eq!(
        f.store.current(&uid("one")).await.unwrap(),
        Participation::Idle
    );
    assert_eq!(f.count("rating_slots").await, 2);
    assert_eq!(
        f.store
            .join(uid("one"), TicketId::new(), POOL)
            .await
            .unwrap()
            .ticket
            .state,
        TicketState::Refused(RefusalReason::RatingPending)
    );
    let next = f.join("one", Pool::Byoyomi30).await;
    f.store.cancel(&uid("one"), one).await.unwrap();
    assert!(matches!(
        f.store.ticket(&uid("one"), next).await.unwrap().state,
        TicketState::Waiting(_)
    ));
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。レーティングのロックを明示的に保持し、有効期限を強制的に切らす。",
        reason: "レーティングのロックで待機中にチケットが失効すると、tick は None を返し、対局を作らない。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn expiry_after_rating_lock_wait_prevents_match() {
    let f = Fixture::new().await;
    let one = f.join("one", POOL).await;
    f.join("two", POOL).await;
    f.raw.execute_unprepared("UPDATE matchmaking_tickets SET expires_at=floor(extract(epoch FROM clock_timestamp())*1000)::bigint+150").await.unwrap();
    let tx = f.raw.begin().await.unwrap();
    tx.execute_unprepared("SELECT * FROM rating_states ORDER BY user_id FOR UPDATE")
        .await
        .unwrap();
    let store = f.store.clone();
    let pending = tokio::spawn(async move { store.tick(POOL).await });
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    tx.commit().await.unwrap();
    assert_eq!(pending.await.unwrap().unwrap(), None);
    assert_eq!(f.count("games").await, 0);
    assert_eq!(
        f.store.ticket(&uid("one"), one).await.unwrap().state,
        TicketState::Expired
    );
}
}

crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。キュー追加時刻を強制的に古くしてから tick を直接呼ぶ。",
        reason: "キュー追加時刻を過去に変更すると、先頭の不適合な参加者を待機させたまま、一人目と二人目が組み合わされる。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn widening_without_a_new_join_skips_an_incompatible_head() {
    let f = Fixture::new().await;
    f.raw.execute_unprepared("INSERT INTO rating_states(user_id,pool,rating,completed) VALUES ('head','ten_minute_30',3000,1),('one','ten_minute_30',1500,1),('two','ten_minute_30',1650,1)").await.unwrap();
    f.join("head", POOL).await;
    f.join("one", POOL).await;
    f.join("two", POOL).await;
    assert!(f.store.tick(POOL).await.unwrap().is_none());
    f.raw
        .execute_unprepared(
            "UPDATE matchmaking_tickets SET enqueued_at=enqueued_at-30000 WHERE state='waiting'",
        )
        .await
        .unwrap();
    let game = f.store.tick(POOL).await.unwrap().unwrap();
    f.ready("one").await;
    f.ready("two").await;
    let record = f.store.load_game(&game).await.unwrap();
    assert!(record.participants.side(&uid("one")).is_ok());
    assert!(record.participants.side(&uid("two")).is_ok());
    assert!(
        matches!(f.store.current(&uid("head")).await.unwrap(),Participation::Active(t) if matches!(t.state,TicketState::Waiting(_)))
    );
}
}

#[tokio::test]
async fn lost_observations_recover_and_worker_retries_preserve_the_next_match() {
    use game_application::rating::{ApplyOutcome, RatingJobRepository, WorkerId, WorkerPolicy};
    use game_persistence::SeaOrmRatingJobs;
    use std::{num::NonZeroU32, time::Duration};
    let f = Fixture::new().await;
    let one = TicketId::new();
    let two = TicketId::new();
    let _ = f.store.join(uid("one"), one, POOL).await.unwrap();
    let recovered = SeaOrmStore::connect(f._db.url()).await.unwrap();
    assert!(!recovered.join(uid("one"), one, POOL).await.unwrap().created);
    f.store.join(uid("two"), two, POOL).await.unwrap();
    let _ = f.store.tick(POOL).await.unwrap();
    recovered.ready(&uid("one"), one).await.unwrap();
    f.store.ready(&uid("two"), two).await.unwrap();
    let game = match recovered.ticket(&uid("one"), one).await.unwrap().state {
        TicketState::Playing { game_id, .. } => game_id,
        other => panic!("{other:?}"),
    };
    let original = f.store.load_game(&game).await.unwrap();
    GameService::new(f.store.clone())
        .resign(
            &game,
            Resign {
                request_id: RequestId::new("finish").unwrap(),
                user_id: original.participants.white().clone(),
                expected_seq: 0,
            },
        )
        .await
        .unwrap();
    let jobs = SeaOrmRatingJobs::connect(f._db.url()).await.unwrap();
    let policy = WorkerPolicy::new(
        Duration::from_secs(30),
        NonZeroU32::new(3).unwrap(),
        Duration::ZERO,
    )
    .unwrap();
    let claim = jobs
        .claim(&WorkerId::new("worker").unwrap(), &policy)
        .await
        .unwrap()
        .unwrap();
    let input = claim.input().unwrap();
    let change =
        rating_domain::calculate(input.game.black_start, input.game.white_start, input.winner)
            .unwrap();
    assert_eq!(
        jobs.apply(&claim, &change).await.unwrap(),
        ApplyOutcome::Applied
    );
    f.join("one", POOL).await;
    f.join("two", POOL).await;
    let next = f.store.tick(POOL).await.unwrap().unwrap();
    f.ready("one").await;
    f.ready("two").await;
    let frozen = pg_test_support::rated_identity(f._db.url(), &next)
        .await
        .unwrap();
    for user in [original.participants.black(), original.participants.white()] {
        let expected = if user == original.participants.black() {
            change.black
        } else {
            change.white
        };
        let actual = if user == frozen.participants.black() {
            frozen.black_start
        } else {
            frozen.white_start
        };
        assert_eq!(actual, expected);
    }
    assert_eq!(
        jobs.apply(&claim, &change).await.unwrap(),
        ApplyOutcome::AlreadyApplied
    );
    assert_eq!(f.count("rating_slots").await, 2);
    assert!(
        matches!(f.store.current(&uid("one")).await.unwrap(),Participation::Active(t) if matches!(t.state,TicketState::Playing {ref game_id,..} if game_id==&next))
    );
}
