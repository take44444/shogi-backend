crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Concurrent real PostgreSQL mutation between reader statements tests snapshot consistency.",
        reason: "A second connection changes metadata and history while history reading is blocked; returned detail retains the prior consistent snapshot.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
