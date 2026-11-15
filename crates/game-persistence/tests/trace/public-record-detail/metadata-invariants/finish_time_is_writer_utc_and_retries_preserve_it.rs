#[tokio::test]
async fn finish_time_is_writer_utc_and_retries_preserve_it() {
    fn micros() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros()
            .try_into()
            .unwrap()
    }
    let f = Fixture::new().await;
    let key = WriteId::new("timed").unwrap();
    f.games
        .seed(
            key.clone(),
            Participants::new(UserId::new("black").unwrap(), UserId::new("white").unwrap())
                .unwrap(),
        )
        .await
        .unwrap();
    let command = Resign {
        request_id: RequestId::new("finish").unwrap(),
        expected_seq: 0,
        user_id: UserId::new("white").unwrap(),
    };
    let before = micros();
    f.games.resign(&key, command.clone()).await.unwrap();
    let after = micros();
    let reader = f.reader().await;
    let id = GameId::new("timed").unwrap();
    let result = reader.detail(&id).await.unwrap();
    assert!((before..=after).contains(&result.metadata.finished_at().get()));
    assert_eq!(result.metadata.final_seq().get(), 1);
    assert_eq!(result.metadata.final_move_ply().get(), 0);
    f.games.resign(&key, command).await.unwrap();
    assert_eq!(reader.detail(&id).await.unwrap(), result);
}
