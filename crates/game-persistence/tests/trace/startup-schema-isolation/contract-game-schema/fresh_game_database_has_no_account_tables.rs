#[tokio::test]
async fn fresh_game_database_has_no_account_tables() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let (_store, _service) = setup(&url).await;
    let raw = Database::connect(&url).await.unwrap();
    assert_eq!(
        integer_query(
            &raw,
            "SELECT COUNT(*) AS value FROM information_schema.tables WHERE table_schema = current_schema() AND table_name = 'users'"
        )
        .await,
        0
    );
}
