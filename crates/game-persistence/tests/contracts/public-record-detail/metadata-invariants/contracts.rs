use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-persistence/writer-database", relationship: crate::trace_support::Relationship::Asserts, scope: "実 PostgreSQL と実保存アダプターの公開 API または DB 接続を使用する。", reason: "対応しないスキーマを変更しないこと、または保存時の不整合拒否と読み取り結果を直接検査する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["postgres"], fake: &[] }] };
#[tokio::test]
async fn postgres_enforces_result_pairs_foreign_keys_and_distinct_participants() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let (_, service) = setup(&url).await;
    service
        .make_move(&id(), movement("m", 0, "black", "7g7f"))
        .await
        .unwrap();
    let raw = Database::connect(&url).await.unwrap();
    for sql in [
        "UPDATE games SET winner = 1, reason = NULL",
        "UPDATE games SET winner = NULL, reason = 2",
        "UPDATE journal SET winner = 1, reason = NULL",
        "UPDATE journal SET winner = NULL, reason = 2",
        "UPDATE games SET black = white",
        "DELETE FROM games WHERE id = 'game'",
        "UPDATE journal SET game_id = 'absent'",
        "DELETE FROM journal WHERE seq = 1",
    ] {
        assert!(raw.execute_unprepared(sql).await.is_err(), "{sql}");
    }
    assert_eq!(service.snapshot(&id()).await.unwrap().seq, 1);
}
}
