use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/game-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL、SeaOrmStore、アプリケーション操作を使う。ブラウザーや RPC 通信は通らない。実際の SQL とコミットの失敗を注入する。", reason: "実際の文実行やコミットにエラーを注入しても、メタデータと受理記録の両方が変更されない。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
#[tokio::test]
async fn real_statement_and_commit_failures_roll_back_both_metadata_and_receipt() {
    for commit_failure in [false, true] {
        let database = TestDatabase::new().await;
        let url = database.url().to_string();
        let (store, service) = setup(&url).await;
        let raw = Database::connect(&url).await.unwrap();
        if commit_failure {
            raw.execute_unprepared("CREATE TABLE deferred_failure (game_id TEXT REFERENCES games(id) DEFERRABLE INITIALLY DEFERRED)").await.unwrap();
            raw.execute_unprepared("CREATE FUNCTION fail_commit_change() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN INSERT INTO deferred_failure VALUES ('absent'); RETURN NEW; END $$; CREATE TRIGGER fail_commit_change AFTER INSERT ON journal FOR EACH ROW EXECUTE FUNCTION fail_commit_change()").await.unwrap();
        } else {
            raw.execute_unprepared("CREATE FUNCTION fail_commit_change() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected write failure'; END $$; CREATE TRIGGER fail_commit_change BEFORE INSERT ON journal FOR EACH ROW EXECUTE FUNCTION fail_commit_change()").await.unwrap();
        }
        let before = service.snapshot(&id()).await.unwrap();
        let command = movement("retry", 0, "black", "7g7f");
        assert_eq!(
            service.make_move(&id(), command.clone()).await,
            Err(Error::Unavailable)
        );
        assert_eq!(store.version(&id()).await.unwrap(), 0);
        assert!(store.load_game(&id()).await.unwrap().journal.is_empty());
        assert_eq!(service.snapshot(&id()).await.unwrap(), before);
        raw.execute_unprepared("DROP TRIGGER fail_commit_change ON journal")
            .await
            .unwrap();
        assert_eq!(service.make_move(&id(), command).await.unwrap().seq, 1);
    }
}
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/game-store", relationship: crate::trace_support::Relationship::Unreviewed, scope: "製品の公開 API を呼ぶ結合テスト。個々の assertion と契約の対応は未レビュー。", reason: "宣言した実提供元の境界を利用する。契約の各条件を直接検証するかは未レビュー。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
#[tokio::test]
async fn direct_store_constraints_and_sequence_overflow_leave_no_changes() {
    let database = TestDatabase::new().await;
    let (store, service) = setup(database.url()).await;
    let receipt = Receipt {
        request: MutationRequest {
            request_id: RequestId::new("r").unwrap(),
            user_id: user("outsider"),
            expected_seq: 0,
            action: Action::Move("7g7f".parse().unwrap()),
        },
        acceptance: Acceptance {
            seq: 1,
            in_check: false,
            result: None,
        },
    };
    assert_eq!(
        store
            .commit_change(&StagedGameChange::new(id(), receipt.clone()).unwrap())
            .await,
        Err(Error::NonParticipant)
    );
    assert_eq!(store.version(&id()).await.unwrap(), 0);
    let mut bad = receipt.clone();
    bad.request.expected_seq = u64::MAX;
    bad.acceptance.seq = 0;
    assert_eq!(
        StagedGameChange::new(id(), bad),
        Err(Error::SequenceExhausted)
    );
    assert_eq!(store.version(&id()).await.unwrap(), 0);
    assert!(store.load_game(&id()).await.unwrap().journal.is_empty());
    service
        .make_move(&id(), movement("r", 0, "black", "7g7f"))
        .await
        .unwrap();
    let mut duplicate = receipt;
    duplicate.request.user_id = user("white");
    duplicate.request.expected_seq = 1;
    duplicate.acceptance.seq = 2;
    duplicate.request.action = Action::Move("3c3d".parse().unwrap());
    assert_eq!(
        store
            .commit_change(&StagedGameChange::new(id(), duplicate).unwrap())
            .await,
        Err(Error::Unavailable)
    );
    assert_eq!(store.version(&id()).await.unwrap(), 1);
    assert_eq!(store.load_game(&id()).await.unwrap().journal.len(), 1);
}
}
