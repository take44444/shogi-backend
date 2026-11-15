crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Real PostgreSQL structural history boundary; intentionally does not replay legality.",
        reason: "All 4096 structural operations are accepted, while 4097 are rejected without replaying board legality.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
