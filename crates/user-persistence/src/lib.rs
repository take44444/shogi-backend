//! Independent PostgreSQL account database. Issuer/subject is the only external identity key.
use async_trait::async_trait;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbBackend,
    QueryResult, Statement, TransactionTrait, Value,
};
use user_application::{
    AccountRepository, DisplayName, Error, LoginAttempt, NewSession, Profile, UserId,
    VerifiedIdentity,
};

pub struct PostgresAccounts {
    db: DatabaseConnection,
}
fn stmt(sql: &str, values: Vec<Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Postgres, sql, values)
}
fn storage(_: impl std::fmt::Debug) -> Error {
    Error::Unavailable
}
fn decode(row: QueryResult) -> Result<Profile, Error> {
    Ok(Profile {
        id: UserId::new(row.try_get::<String>("", "id").map_err(storage)?)
            .map_err(|_| Error::Corrupt)?,
        display_name: DisplayName::new(row.try_get("", "display_name").map_err(storage)?)
            .map_err(|_| Error::Corrupt)?,
        created_at: row.try_get("", "created_at").map_err(storage)?,
    })
}
async fn schema_fingerprint(tx: &DatabaseTransaction, schema: &str) -> Result<Vec<String>, Error> {
    let rows = tx
        .query_all_raw(stmt(
            r#"
        SELECT definition FROM (
            SELECT format('relation|%s|%s|%s|%s|%s', c.relname, c.relkind,
                          c.relpersistence, c.relrowsecurity, c.relforcerowsecurity) AS definition
            FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE n.nspname::text = $1 AND c.relkind <> 'i'
            UNION ALL
            SELECT format('column|%s|%s|%s|%s|%s|%s|%s|%s|%s', c.relname, a.attnum,
                          a.attname, format_type(a.atttypid, a.atttypmod), a.attnotnull,
                          COALESCE(pg_get_expr(d.adbin, d.adrelid), ''),
                          a.attidentity, a.attgenerated, COALESCE(coll.collname, ''))
            FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            LEFT JOIN pg_attrdef d ON d.adrelid = c.oid AND d.adnum = a.attnum
            LEFT JOIN pg_collation coll ON coll.oid = a.attcollation
            WHERE n.nspname::text = $1 AND a.attnum > 0 AND NOT a.attisdropped AND c.relkind <> 'i'
            UNION ALL
            SELECT format('constraint|%s|%s|%s|%s|%s|%s', c.relname, k.conname,
                          replace(pg_get_constraintdef(k.oid), quote_ident($1) || '.', ''),
                          k.condeferrable, k.condeferred, k.convalidated)
            FROM pg_constraint k JOIN pg_class c ON c.oid = k.conrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE n.nspname::text = $1
            UNION ALL
            SELECT format('index|%s|%s|%s|%s', c.relname,
                          replace(pg_get_indexdef(c.oid), quote_ident($1) || '.', ''),
                          i.indisvalid, i.indisready)
            FROM pg_class c JOIN pg_index i ON i.indexrelid = c.oid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE n.nspname::text = $1
        ) definitions ORDER BY definition
        "#,
            vec![schema.into()],
        ))
        .await
        .map_err(storage)?;
    rows.into_iter()
        .map(|row| row.try_get("", "definition").map_err(|_| Error::Corrupt))
        .collect()
}
const SCHEMA: &[&str] = &[
    "CREATE TABLE account_schema_version (version INTEGER NOT NULL PRIMARY KEY CHECK (version = 1))",
    "CREATE TABLE accounts (id TEXT PRIMARY KEY NOT NULL, display_name TEXT NOT NULL, created_at BIGINT NOT NULL)",
    "CREATE TABLE external_identities (issuer TEXT NOT NULL, subject TEXT NOT NULL, user_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE, PRIMARY KEY (issuer, subject))",
    "CREATE TABLE login_attempts (state_hash TEXT PRIMARY KEY NOT NULL, browser_hash TEXT NOT NULL, nonce TEXT NOT NULL, pkce_verifier TEXT NOT NULL, expires_at BIGINT NOT NULL)",
    "CREATE TABLE sessions (token_hash TEXT PRIMARY KEY NOT NULL, user_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE, expires_at BIGINT NOT NULL)",
    "CREATE INDEX sessions_expiry ON sessions(expires_at)",
    "CREATE INDEX attempts_expiry ON login_attempts(expires_at)",
    "INSERT INTO account_schema_version VALUES (1)",
];
impl PostgresAccounts {
    pub async fn connect(url: &str) -> Result<Self, Error> {
        if !(url.starts_with("postgres://") || url.starts_with("postgresql://")) {
            return Err(Error::InvalidInput);
        }
        let mut opts = ConnectOptions::new(url);
        opts.max_connections(5)
            .sqlx_logging(false)
            .map_sqlx_postgres_opts(|o| o.options([("search_path", "public")]));
        let db = Database::connect(opts).await.map_err(storage)?;
        let tx = db.begin().await.map_err(storage)?;
        tx.query_one_raw(stmt(
            "SELECT pg_advisory_xact_lock(732601005046175415)",
            vec![],
        ))
        .await
        .map_err(storage)?;
        let actual = schema_fingerprint(&tx, "public").await?;
        if actual.is_empty() {
            for sql in SCHEMA {
                tx.execute_raw(stmt(sql, vec![])).await.map_err(storage)?;
            }
        } else {
            let version = tx
                .query_all_raw(stmt("SELECT version FROM account_schema_version", vec![]))
                .await
                .map_err(|_| Error::Corrupt)?;
            if version.len() != 1
                || version[0]
                    .try_get::<i32>("", "version")
                    .map_err(|_| Error::Corrupt)?
                    != 1
            {
                return Err(Error::Corrupt);
            }
            let expected_schema = format!("account_expected_{}", uuid::Uuid::new_v4().simple());
            tx.execute_raw(stmt(&format!("CREATE SCHEMA {expected_schema}"), vec![]))
                .await
                .map_err(storage)?;
            tx.execute_raw(stmt(
                &format!("SET LOCAL search_path TO {expected_schema}"),
                vec![],
            ))
            .await
            .map_err(storage)?;
            for sql in &SCHEMA[..SCHEMA.len() - 1] {
                tx.execute_raw(stmt(sql, vec![])).await.map_err(storage)?;
            }
            let expected = schema_fingerprint(&tx, &expected_schema).await?;
            if actual != expected {
                return Err(Error::Corrupt);
            }
            tx.execute_raw(stmt("SET LOCAL search_path TO public", vec![]))
                .await
                .map_err(storage)?;
            tx.execute_raw(stmt(
                &format!("DROP SCHEMA {expected_schema} CASCADE"),
                vec![],
            ))
            .await
            .map_err(storage)?;
        }
        tx.commit().await.map_err(storage)?;
        Ok(Self { db })
    }
    pub async fn close(&self) -> Result<(), Error> {
        self.db.close_by_ref().await.map_err(storage)
    }
}
const AUTH: &str = "SELECT a.id,a.display_name,a.created_at FROM accounts a JOIN sessions s ON s.user_id=a.id WHERE s.token_hash=$1 AND s.expires_at>$2";
#[async_trait]
impl AccountRepository for PostgresAccounts {
    async fn save_attempt(&self, a: LoginAttempt, now: i64) -> Result<(), Error> {
        let tx = self.db.begin().await.map_err(storage)?;
        tx.execute_raw(stmt(
            "DELETE FROM login_attempts WHERE expires_at<=$1",
            vec![now.into()],
        ))
        .await
        .map_err(storage)?;
        tx.execute_raw(stmt(
            "DELETE FROM sessions WHERE expires_at<=$1",
            vec![now.into()],
        ))
        .await
        .map_err(storage)?;
        tx.execute_raw(stmt(
            "INSERT INTO login_attempts VALUES ($1,$2,$3,$4,$5)",
            vec![
                a.state_hash.into(),
                a.browser_hash.into(),
                a.nonce.into(),
                a.pkce_verifier.into(),
                a.expires_at.into(),
            ],
        ))
        .await
        .map_err(storage)?;
        tx.commit().await.map_err(storage)
    }
    async fn consume_attempt(
        &self,
        state: &str,
        browser: &str,
        now: i64,
    ) -> Result<LoginAttempt, Error> {
        let row = self.db.query_one_raw(stmt("DELETE FROM login_attempts WHERE state_hash=$1 AND browser_hash=$2 AND expires_at>$3 RETURNING state_hash,browser_hash,nonce,pkce_verifier,expires_at", vec![state.into(), browser.into(), now.into()])).await.map_err(storage)?.ok_or(Error::Unauthorized)?;
        Ok(LoginAttempt {
            state_hash: row.try_get("", "state_hash").map_err(storage)?,
            browser_hash: row.try_get("", "browser_hash").map_err(storage)?,
            nonce: row.try_get("", "nonce").map_err(storage)?,
            pkce_verifier: row.try_get("", "pkce_verifier").map_err(storage)?,
            expires_at: row.try_get("", "expires_at").map_err(storage)?,
        })
    }
    async fn login(
        &self,
        identity: VerifiedIdentity,
        session: NewSession,
        now: i64,
    ) -> Result<Profile, Error> {
        let tx = self.db.begin().await.map_err(storage)?;
        let identity_key = format!(
            "{}:{}{}",
            identity.issuer.len(),
            identity.issuer,
            identity.subject
        );
        tx.query_one_raw(stmt(
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            vec![identity_key.into()],
        ))
        .await
        .map_err(storage)?;
        let existing = tx.query_one_raw(stmt("SELECT a.id,a.display_name,a.created_at FROM accounts a JOIN external_identities e ON e.user_id=a.id WHERE e.issuer=$1 AND e.subject=$2", vec![identity.issuer.clone().into(), identity.subject.clone().into()])).await.map_err(storage)?;
        let profile = if let Some(row) = existing {
            decode(row)?
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute_raw(stmt(
                "INSERT INTO accounts VALUES ($1,$2,$3)",
                vec![
                    id.clone().into(),
                    identity.initial_name.as_str().into(),
                    now.into(),
                ],
            ))
            .await
            .map_err(storage)?;
            tx.execute_raw(stmt(
                "INSERT INTO external_identities VALUES ($1,$2,$3)",
                vec![
                    identity.issuer.into(),
                    identity.subject.into(),
                    id.clone().into(),
                ],
            ))
            .await
            .map_err(storage)?;
            Profile {
                id: UserId::new(id).map_err(|_| Error::Corrupt)?,
                display_name: identity.initial_name,
                created_at: now,
            }
        };
        tx.execute_raw(stmt(
            "INSERT INTO sessions VALUES ($1,$2,$3)",
            vec![
                session.token_hash.into(),
                profile.id.as_str().into(),
                session.expires_at.into(),
            ],
        ))
        .await
        .map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(profile)
    }
    async fn authenticate(&self, hash: &str, now: i64) -> Result<Profile, Error> {
        decode(
            self.db
                .query_one_raw(stmt(AUTH, vec![hash.into(), now.into()]))
                .await
                .map_err(storage)?
                .ok_or(Error::Unauthorized)?,
        )
    }
    async fn rename(&self, hash: &str, name: DisplayName, now: i64) -> Result<Profile, Error> {
        let tx = self.db.begin().await.map_err(storage)?;
        let mut p = decode(
            tx.query_one_raw(stmt(AUTH, vec![hash.into(), now.into()]))
                .await
                .map_err(storage)?
                .ok_or(Error::Unauthorized)?,
        )?;
        tx.execute_raw(stmt(
            "UPDATE accounts SET display_name=$1 WHERE id=$2",
            vec![name.as_str().into(), p.id.as_str().into()],
        ))
        .await
        .map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        p.display_name = name;
        Ok(p)
    }
    async fn logout(&self, hash: &str) -> Result<(), Error> {
        self.db
            .execute_raw(stmt(
                "DELETE FROM sessions WHERE token_hash=$1",
                vec![hash.into()],
            ))
            .await
            .map_err(storage)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> VerifiedIdentity {
        VerifiedIdentity {
            issuer: "https://accounts.google.com".into(),
            subject: "subject".into(),
            initial_name: DisplayName::new("Player".into()).unwrap(),
        }
    }
    fn session(hash: &str, expiry: i64) -> NewSession {
        NewSession {
            token_hash: hash.into(),
            expires_at: expiry,
        }
    }
    fn attempt(state: &str, expiry: i64) -> LoginAttempt {
        LoginAttempt {
            state_hash: state.into(),
            browser_hash: "browser-hash".into(),
            nonce: "nonce".into(),
            pkce_verifier: "verifier".into(),
            expires_at: expiry,
        }
    }
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
            a.db.query_one_raw(stmt("SELECT COUNT(*) AS n FROM accounts", vec![]))
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
            .query_one_raw(stmt("SELECT COUNT(*) AS n FROM accounts", vec![]))
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
    #[tokio::test]
    async fn missing_ledger_or_changed_indexes_fail_closed() {
        for mutation in [
            "DELETE FROM account_schema_version",
            "DROP TABLE account_schema_version",
            "DROP INDEX sessions_expiry",
            "ALTER TABLE accounts ALTER COLUMN display_name DROP NOT NULL",
            "ALTER TABLE external_identities DROP CONSTRAINT external_identities_user_id_fkey",
            "ALTER TABLE account_schema_version DROP CONSTRAINT account_schema_version_version_check",
        ] {
            let database = pg_test_support::TestDatabase::new().await;
            let store = PostgresAccounts::connect(database.url()).await.unwrap();
            store.db.execute_unprepared(mutation).await.unwrap();
            store.close().await.unwrap();
            assert!(
                matches!(
                    PostgresAccounts::connect(database.url()).await,
                    Err(Error::Corrupt)
                ),
                "{mutation}"
            );
        }
    }
    #[tokio::test]
    async fn schema_constraints_and_ledger_are_checked_on_reopen() {
        let database = pg_test_support::TestDatabase::new().await;
        let url = database.url();
        let store = PostgresAccounts::connect(url).await.unwrap();
        store
            .db
            .execute_unprepared("DROP TABLE external_identities")
            .await
            .unwrap();
        store
            .db
            .execute_unprepared(
                "CREATE TABLE external_identities (issuer TEXT, subject TEXT, user_id TEXT)",
            )
            .await
            .unwrap();
        store.close().await.unwrap();
        assert!(matches!(
            PostgresAccounts::connect(url).await,
            Err(Error::Corrupt)
        ));
    }
}
