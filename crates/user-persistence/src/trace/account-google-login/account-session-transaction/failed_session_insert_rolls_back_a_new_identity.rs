crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実 PostgreSQL の重複 session key による失敗と再試行を確認する。HTTP のエラー変換は通らない。",
        reason: "新 subject のセッション挿入を重複キーで失敗させ、accounts が増えず、別 token で再試行でき、元 token の本人識別が維持されることを確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
    async fn failed_session_insert_rolls_back_a_new_identity() {
        let database = pg_test_support::TestDatabase::new().await;
        let store = PostgresAccounts::connect(database.url()).await.unwrap();
        let first = store
            .login(identity(), session("duplicate", 200), 100)
            .await
            .unwrap();
        let mut other = identity();
        other.subject = "other-subject".into();
        assert_eq!(
            store.login(other, session("duplicate", 200), 100).await,
            Err(Error::Unavailable)
        );
        let rows = store
            .db
            .query_one(
                Query::select()
                    .expr_as(Expr::col(Accounts::Id).count(), "n")
                    .from(Accounts::Table),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(rows.try_get::<i64>("", "n").unwrap(), 1);
        let mut other = identity();
        other.subject = "other-subject".into();
        let second = store
            .login(other, session("second", 200), 100)
            .await
            .unwrap();
        assert_ne!(first.id, second.id);
        assert_eq!(store.authenticate("duplicate", 100).await.unwrap(), first);
    }
}
