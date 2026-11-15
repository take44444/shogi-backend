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
        .seed(
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
