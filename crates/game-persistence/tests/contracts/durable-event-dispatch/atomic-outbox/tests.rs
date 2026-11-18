use super::*;

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/game-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL の書き込みと Outbox の永続化、再試行、拒否、復元、固定されたイベント内容を検証する。", reason: "実データベースで、再試行・拒否・復元後のイベント数を確認し、履歴を書き換えてもイベント内容が固定されたままであることを検証する。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }, crate::trace_support::Evidence { contract: "game-application/outbox-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL の書き込みと Outbox の永続化、再試行、拒否、復元、固定されたイベント内容を検証する。", reason: "実際の PostgreSQL は、シーケンス 2 の終局より先に、固定された着手を返す。最初の行を二度処理済みにしても、終局イベントは未処理のまま残る。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
#[tokio::test]
    async fn retry_rejection_restoration_and_frozen_payload() {
        let database = TestDatabase::new().await;
        let url = database.url();
        let (store, service, raw) = setup(url).await;
        assert_eq!(count(&raw, "outbox_events").await, 0);
        assert!(
            service
                .make_move(
                    &id(),
                    MakeMove {
                        user_id: user("white"),
                        ..command()
                    }
                )
                .await
                .is_err()
        );
        assert_eq!(count(&raw, "outbox_events").await, 0);
        let accepted = service.make_move(&id(), command()).await.unwrap();
        assert_eq!(service.make_move(&id(), command()).await.unwrap(), accepted);
        let restored = pg_test_support::GameFixture::new(store.clone(), database.url());
        assert_eq!(restored.snapshot(&id()).await.unwrap().seq, 1);
        assert_eq!(
            restored.make_move(&id(), command()).await.unwrap(),
            accepted
        );
        assert_eq!(count(&raw, "outbox_events").await, 1);
        service
            .resign(
                &id(),
                Resign {
                    request_id: RequestId::new("end").unwrap(),
                    expected_seq: 1,
                    user_id: user("white"),
                },
            )
            .await
            .unwrap();
        assert_eq!(count(&raw, "outbox_events").await, 2);
        raw.execute_unprepared("UPDATE journal SET usi = '2g2f' WHERE seq = 1")
            .await
            .unwrap();
        let mut queue = PostgresOutbox::connect(url).await.unwrap();
        let item = queue.next_pending().await.unwrap().unwrap();
        assert!(
            matches!(item.event.event, DomainEvent::MoveCommitted { movement, .. } if movement.to_string() == "7g7f")
        );
        queue.mark_published(&item).await.unwrap();
        queue.mark_published(&item).await.unwrap();
        let end = queue.next_pending().await.unwrap().unwrap();
        assert_eq!(end.event.seq, 2);
        assert!(matches!(end.event.event, DomainEvent::GameFinished { .. }));
        queue.mark_published(&end).await.unwrap();
        assert!(queue.next_pending().await.unwrap().is_none());
    }
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/game-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL で、トリガーによるキュー追加失敗の注入とロールバックを検証する。", reason: "キューへの追加を失敗させるトリガーによって、対局メタデータ・履歴・Outbox をロールバックする。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
#[tokio::test]
    async fn failed_enqueue_rolls_back_metadata_journal_and_events() {
        let database = TestDatabase::new().await;
        let url = database.url();
        let (store, service, raw) = setup(url).await;
        raw.execute_unprepared("CREATE FUNCTION fail_outbox() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected outbox failure'; END $$; CREATE TRIGGER fail_outbox BEFORE INSERT ON outbox_events FOR EACH ROW EXECUTE FUNCTION fail_outbox()").await.unwrap();
        assert_eq!(
            service.make_move(&id(), command()).await,
            Err(Error::Unavailable)
        );
        assert_eq!(store.version(&id()).await.unwrap(), 0);
        assert_eq!(count(&raw, "journal").await, 0);
        assert_eq!(count(&raw, "outbox_events").await, 0);
        raw.execute_unprepared("DROP TRIGGER fail_outbox ON outbox_events")
            .await
            .unwrap();
        assert_eq!(service.make_move(&id(), command()).await.unwrap().seq, 1);
        assert_eq!(count(&raw, "outbox_events").await, 1);
    }
}

crate::trace_support::boundary_case! { crate::trace_support::BoundaryCase { evidence: &[crate::trace_support::Evidence { contract: "game-application/game-store", relationship: crate::trace_support::Relationship::Asserts, scope: "実際の PostgreSQL で、成否不明のコミットを注入し、冪等な再試行を検証する。", reason: "実際のコミットの成否が不明となった後に再試行しても、Outbox の行は一件となる。", observation: crate::trace_support::Observation::ProviderBoundary, real: &["game-persistence"], fake: &[] }] };
#[tokio::test]
    async fn uncertain_commit_and_retry_enqueue_once() {
        let database = TestDatabase::new().await;
        let url = database.url();
        let (store, _, raw) = setup(url).await;
        let service = pg_test_support::GameFixture::new(Arc::new(Uncertain(store)), database.url());
        let accepted = service.make_move(&id(), command()).await.unwrap();
        assert_eq!(service.make_move(&id(), command()).await.unwrap(), accepted);
        assert_eq!(count(&raw, "outbox_events").await, 1);
    }
}
