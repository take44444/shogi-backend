use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/game-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。実際のコミットと通常のリポジトリ読み込みを並行実行する。", reason: "書き込みと読み取りの並行ループで、バージョンと履歴長、最終結果と最後の受理記録を比較し、独立したスナップショット復元が成功する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn joined_history_load_remains_coherent_during_concurrent_commits() {
    let database = TestDatabase::new().await;
    let url = database.url().to_string();
    let (_, writer) = setup(&url).await;
    let reader = Arc::new(SeaOrmStore::connect(&url).await.unwrap());
    let writing = tokio::spawn(async move {
        for seq in 0..24 {
            let state = writer.snapshot(&id()).await.unwrap();
            if state.result.is_some() {
                break;
            }
            writer
                .make_move(
                    &id(),
                    movement(
                        &format!("m-{seq}"),
                        seq,
                        if state.turn == Player::Black {
                            "black"
                        } else {
                            "white"
                        },
                        &state.legal_moves[0].to_string(),
                    ),
                )
                .await
                .unwrap();
        }
    });
    for _ in 0..50 {
        let record = reader.load_game(&id()).await.unwrap();
        assert_eq!(record.version, record.journal.len() as u64);
        assert_eq!(
            record.result,
            record
                .journal
                .last()
                .and_then(|receipt| receipt.acceptance.result)
        );
        let service = pg_test_support::GameFixture::new(reader.clone(), &url);
        service.snapshot(&id()).await.unwrap();
    }
    writing.await.unwrap();
}
}
