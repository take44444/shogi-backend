crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "実 PostgreSQL repository に指定時刻を渡し、試行の期限・ブラウザー一致・単回消費を確認する。HTTP は通らない。",
        reason: "期限ちょうどの試行と不一致ブラウザーを Unauthorized とし、有効な試行は一度だけ消費できることを確認する。",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[crate::trace_support::Evidence { feature: "account-profile", contract: "profile-update", scope: "実 PostgreSQL repository に時刻を渡し、期限切れセッションの rename 拒否を確認する。HTTP は通らない。", relationship: crate::trace_support::Relationship::Asserts, reason: "期限ちょうどの session で rename が Unauthorized となることを直接確認する。" }, crate::trace_support::Evidence { feature: "account-session", contract: "session-storage", scope: "実 PostgreSQL に指定時刻を渡す repository の期限・失効検証。HTTP は通らない。", relationship: crate::trace_support::Relationship::Asserts, reason: "期限の一秒前は認証でき、期限ちょうどは Unauthorized、logout 後は以前の時刻を渡しても認証できないことを確認する。" }],
    };
#[tokio::test]
    async fn expired_attempts_and_sessions_cannot_be_used() {
        let database = pg_test_support::TestDatabase::new().await;
        let store = PostgresAccounts::connect(database.url()).await.unwrap();
        store
            .save_attempt(attempt("expired", 20), 10)
            .await
            .unwrap();
        assert!(matches!(
            store.consume_attempt("expired", "browser-hash", 20).await,
            Err(Error::Unauthorized)
        ));
        store.save_attempt(attempt("valid", 30), 20).await.unwrap();
        assert!(matches!(
            store.consume_attempt("valid", "wrong-browser", 20).await,
            Err(Error::Unauthorized)
        ));
        assert!(
            store
                .consume_attempt("valid", "browser-hash", 20)
                .await
                .is_ok()
        );
        assert!(matches!(
            store.consume_attempt("valid", "browser-hash", 20).await,
            Err(Error::Unauthorized)
        ));
        let p = store
            .login(identity(), session("session-hash", 30), 20)
            .await
            .unwrap();
        assert_eq!(store.authenticate("session-hash", 29).await.unwrap(), p);
        assert_eq!(
            store.authenticate("session-hash", 30).await,
            Err(Error::Unauthorized)
        );
        assert_eq!(
            store
                .rename(
                    "session-hash",
                    DisplayName::new("Changed".into()).unwrap(),
                    30
                )
                .await,
            Err(Error::Unauthorized)
        );
        store.logout("session-hash").await.unwrap();
        assert_eq!(
            store.authenticate("session-hash", 20).await,
            Err(Error::Unauthorized)
        );
    }
}
