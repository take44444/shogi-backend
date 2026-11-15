crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実 PostgreSQL の二つの接続で login と consume_attempt を競合させる。HTTP と Google は通らない。",
        reason: "同じ本人識別で同時ログインした結果が一致し、両セッションが同一ユーザーを認証し、accounts が一行であることを確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "account-google-login", contract: "attempt-consumption", scope: "実 PostgreSQL の二つの接続で login と consume_attempt を競合させる。HTTP と Google は通らない。", relationship: crate::trace_support::Relationship::Asserts, reason: "二つの接続から同じ試行を同時消費して、成功が一つだけであることを確認する。" }],
    };
#[tokio::test]
    async fn concurrent_first_logins_and_callback_consumption_converge() {
        let database = pg_test_support::TestDatabase::new().await;
        let url = database.url();
        let (a, b) = tokio::join!(
            PostgresAccounts::connect(url),
            PostgresAccounts::connect(url)
        );
        let a = a.unwrap();
        let b = b.unwrap();
        let (left, right) = tokio::join!(
            a.login(identity(), session("hash-a", 200), 100),
            b.login(identity(), session("hash-b", 200), 100)
        );
        assert_eq!(left.unwrap(), right.unwrap());
        assert_eq!(
            a.authenticate("hash-a", 100).await.unwrap(),
            b.authenticate("hash-b", 100).await.unwrap()
        );
        let rows =
            a.db.query_one(
                Query::select()
                    .expr_as(Expr::col(Accounts::Id).count(), "n")
                    .from(Accounts::Table),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(rows.try_get::<i64>("", "n").unwrap(), 1);
        a.save_attempt(attempt("state", 200), 100).await.unwrap();
        let (left, right) = tokio::join!(
            a.consume_attempt("state", "browser-hash", 100),
            b.consume_attempt("state", "browser-hash", 100)
        );
        assert_eq!(left.is_ok() as u8 + right.is_ok() as u8, 1);
    }
}
