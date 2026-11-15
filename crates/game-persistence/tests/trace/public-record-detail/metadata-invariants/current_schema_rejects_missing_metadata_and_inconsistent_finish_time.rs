#[tokio::test]
async fn current_schema_rejects_missing_metadata_and_inconsistent_finish_time() {
    let f = Fixture::new().await;
    f.completed("game", "black", "white", 1, "white", 20).await;
    for mutation in [
        "rule=NULL",
        "visibility=NULL",
        "final_move_ply=NULL",
        "finished_at=NULL",
        "winner=NULL",
        "reason=NULL",
    ] {
        assert!(
            f.raw
                .execute_unprepared(&format!("UPDATE games SET {mutation} WHERE id='game'"))
                .await
                .is_err(),
            "{mutation}"
        );
    }
}
