use super::*;

#[traceability::targets(
    game_persistence::SeaOrmStore::connect,
    game_record_persistence::PostgresGameRecords::search,
    game_record_persistence::PostgresGameRecords::detail
)]
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
    let store = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
    let games = pg_test_support::GameFixture::new(store.clone(), &url);
    for name in ["current-finished", "current-private", "current-ongoing"] {
        games
            .seed(
                WriteId::new(name).unwrap(),
                Participants::new(UserId::new("black").unwrap(), UserId::new("white").unwrap())
                    .unwrap(),
            )
            .await
            .unwrap();
        games
            .make_move(
                &WriteId::new(name).unwrap(),
                game_application::MakeMove {
                    request_id: RequestId::new("move").unwrap(),
                    expected_seq: 0,
                    user_id: UserId::new("black").unwrap(),
                    movement: "7g7f".parse().unwrap(),
                },
            )
            .await
            .unwrap();
        if name != "current-ongoing" {
            games
                .resign(
                    &WriteId::new(name).unwrap(),
                    Resign {
                        request_id: RequestId::new("end").unwrap(),
                        expected_seq: 1,
                        user_id: UserId::new("white").unwrap(),
                    },
                )
                .await
                .unwrap();
        }
    }
    db.execute_unprepared("UPDATE games SET finished_at=20 WHERE id='current-finished'; UPDATE games SET finished_at=30,visibility='private' WHERE id='current-private'").await.unwrap();
    store.close().await.unwrap();
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
