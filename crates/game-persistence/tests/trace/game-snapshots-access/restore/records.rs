use super::*;

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
    pg_test_support::seed_game(
        &url,
        &WriteId::new("current-ongoing").unwrap(),
        &Participants::new(UserId::new("black").unwrap(), UserId::new("white").unwrap()).unwrap(),
    )
    .await
    .unwrap();
    db.execute_unprepared("UPDATE games SET version=1,final_move_ply=1 WHERE id='current-ongoing'; INSERT INTO games VALUES
        ('current-finished','black','white',2,1,2,'standard','public',20,1),
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
    pg_test_support::GameFixture::new(store.clone(), database.url())
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
