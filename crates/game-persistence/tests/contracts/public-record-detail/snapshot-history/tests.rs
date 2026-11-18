use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-record-application/record-detail-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL で、公開棋譜のフィルターと非公開詳細の検索を検証する。", reason: "リポジトリで直接取得し、非公開の終了済み対局が見つからないことを検証する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-record-persistence"], fake: &[] }, crate::trace_support::Evidence { contract: "game-record-application/record-search-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL の読み取り・書き込み用のテスト構成で、フィルター、公開範囲、キーベースのページングの変化を検証する。", reason: "リポジトリを直接検証し、個別フィルター、組み合わせた条件、同時刻の ID を c/b の順に並べてから古い a を返すこと、まだ見ていない新しい行の除外、新たに非公開となった行の省略を確認する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-record-persistence"], fake: &[] }] };
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
        .seed(
            WriteId::new("ongoing").unwrap(),
            Participants::new(
                UserId::new("ongoing-black").unwrap(),
                UserId::new("ongoing-white").unwrap(),
            )
            .unwrap(),
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
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-record-application/record-detail-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL で、履歴とメタデータの破損、および上限を超える履歴の拒否を検証する。", reason: "八種類の破損変更と上限を超える履歴を拒否し、部分的な詳細は返さない。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-record-persistence"], fake: &[] }] };
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
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-record-application/record-detail-store", relationship: crate::trace_support::Relationship::Asserts, scope: "読み取り文の間で実際の PostgreSQL を並行変更し、スナップショットの整合性を検証する。", reason: "履歴の読み込みが待機している間に別の接続がメタデータと履歴を変更しても、返す詳細は変更前の整合したスナップショットを保持する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-record-persistence"], fake: &[] }] };
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
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-record-application/record-detail-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL の履歴の構造上の境界を検証する。意図的に合法性の再現は行わない。", reason: "盤面上の合法性を再現せず、構造上の操作を 4096 件までは受け入れ、4097 件は拒否する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-record-persistence"], fake: &[] }] };
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
}
