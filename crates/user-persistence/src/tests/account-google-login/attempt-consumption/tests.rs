use super::*;

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
