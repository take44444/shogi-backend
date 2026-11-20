use super::*;

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
