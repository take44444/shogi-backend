//! Independent SQLite account database. Issuer/subject is the only external identity key.
use async_trait::async_trait;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbBackend,
    QueryResult, SqliteTransactionMode, Statement, TransactionOptions, TransactionTrait, Value,
};
use std::time::Duration;
use user_application::{
    AccountRepository, DisplayName, Error, LoginAttempt, NewSession, Profile, UserId,
    VerifiedIdentity,
};

pub struct SqliteAccounts {
    db: DatabaseConnection,
}
fn stmt(sql: &str, values: Vec<Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Sqlite, sql, values)
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
async fn immediate(db: &DatabaseConnection) -> Result<DatabaseTransaction, Error> {
    db.begin_with_options(TransactionOptions {
        sqlite_transaction_mode: Some(SqliteTransactionMode::Immediate),
        ..Default::default()
    })
    .await
    .map_err(storage)
}
const SCHEMA: &[&str] = &[
    "CREATE TABLE account_schema_version (version INTEGER NOT NULL PRIMARY KEY CHECK (version = 1))",
    "CREATE TABLE accounts (id TEXT PRIMARY KEY NOT NULL, display_name TEXT NOT NULL, created_at INTEGER NOT NULL)",
    "CREATE TABLE external_identities (issuer TEXT NOT NULL, subject TEXT NOT NULL, user_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE, PRIMARY KEY (issuer, subject))",
    "CREATE TABLE login_attempts (state_hash TEXT PRIMARY KEY NOT NULL, browser_hash TEXT NOT NULL, nonce TEXT NOT NULL, pkce_verifier TEXT NOT NULL, expires_at INTEGER NOT NULL)",
    "CREATE TABLE sessions (token_hash TEXT PRIMARY KEY NOT NULL, user_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE, expires_at INTEGER NOT NULL)",
    "CREATE INDEX sessions_expiry ON sessions(expires_at)",
    "CREATE INDEX attempts_expiry ON login_attempts(expires_at)",
    "INSERT INTO account_schema_version VALUES (1)",
];
impl SqliteAccounts {
    pub async fn connect(url: &str) -> Result<Self, Error> {
        if !url.starts_with("sqlite:") {
            return Err(Error::InvalidInput);
        }
        let mut opts = ConnectOptions::new(url);
        opts.max_connections(if url.contains(":memory:") { 1 } else { 5 })
            .sqlx_logging(false)
            .map_sqlx_sqlite_opts(|o| {
                o.foreign_keys(true)
                    .pragma("journal_mode", "WAL")
                    .pragma("synchronous", "FULL")
                    .busy_timeout(Duration::from_secs(5))
            });
        let db = Database::connect(opts).await.map_err(storage)?;
        let tx = immediate(&db).await?;
        let tables = tx
            .query_all_raw(stmt(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                vec![],
            ))
            .await
            .map_err(storage)?;
        if tables.is_empty() {
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
                    .try_get::<i64>("", "version")
                    .map_err(|_| Error::Corrupt)?
                    != 1
            {
                return Err(Error::Corrupt);
            }
            let definitions = tx.query_all_raw(stmt("SELECT sql FROM sqlite_master WHERE type IN ('table','index') AND name NOT LIKE 'sqlite_%' AND sql IS NOT NULL", vec![])).await.map_err(storage)?;
            let mut actual: Vec<String> = definitions
                .into_iter()
                .map(|r| r.try_get("", "sql").map_err(|_| Error::Corrupt))
                .collect::<Result<_, _>>()?;
            let mut expected: Vec<String> = SCHEMA[..SCHEMA.len() - 1]
                .iter()
                .map(|s| s.to_string())
                .collect();
            actual.sort();
            expected.sort();
            if actual != expected {
                return Err(Error::Corrupt);
            }
        }
        tx.commit().await.map_err(storage)?;
        Ok(Self { db })
    }
    pub async fn close(&self) -> Result<(), Error> {
        self.db.close_by_ref().await.map_err(storage)
    }
}
const AUTH: &str = "SELECT a.id,a.display_name,a.created_at FROM accounts a JOIN sessions s ON s.user_id=a.id WHERE s.token_hash=? AND s.expires_at>?";
#[async_trait]
impl AccountRepository for SqliteAccounts {
    async fn save_attempt(&self, a: LoginAttempt, now: i64) -> Result<(), Error> {
        let tx = immediate(&self.db).await?;
        tx.execute_raw(stmt(
            "DELETE FROM login_attempts WHERE expires_at<=?",
            vec![now.into()],
        ))
        .await
        .map_err(storage)?;
        tx.execute_raw(stmt(
            "DELETE FROM sessions WHERE expires_at<=?",
            vec![now.into()],
        ))
        .await
        .map_err(storage)?;
        tx.execute_raw(stmt(
            "INSERT INTO login_attempts VALUES (?,?,?,?,?)",
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
        let row = self.db.query_one_raw(stmt("DELETE FROM login_attempts WHERE state_hash=? AND browser_hash=? AND expires_at>? RETURNING state_hash,browser_hash,nonce,pkce_verifier,expires_at", vec![state.into(), browser.into(), now.into()])).await.map_err(storage)?.ok_or(Error::Unauthorized)?;
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
        let tx = immediate(&self.db).await?;
        let existing = tx.query_one_raw(stmt("SELECT a.id,a.display_name,a.created_at FROM accounts a JOIN external_identities e ON e.user_id=a.id WHERE e.issuer=? AND e.subject=?", vec![identity.issuer.clone().into(), identity.subject.clone().into()])).await.map_err(storage)?;
        let profile = if let Some(row) = existing {
            decode(row)?
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute_raw(stmt(
                "INSERT INTO accounts VALUES (?,?,?)",
                vec![
                    id.clone().into(),
                    identity.initial_name.as_str().into(),
                    now.into(),
                ],
            ))
            .await
            .map_err(storage)?;
            tx.execute_raw(stmt(
                "INSERT INTO external_identities VALUES (?,?,?)",
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
            "INSERT INTO sessions VALUES (?,?,?)",
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
        let tx = immediate(&self.db).await?;
        let mut p = decode(
            tx.query_one_raw(stmt(AUTH, vec![hash.into(), now.into()]))
                .await
                .map_err(storage)?
                .ok_or(Error::Unauthorized)?,
        )?;
        tx.execute_raw(stmt(
            "UPDATE accounts SET display_name=? WHERE id=?",
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
                "DELETE FROM sessions WHERE token_hash=?",
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
        let store = SqliteAccounts::connect("sqlite::memory:").await.unwrap();
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
        let temp = tempfile::tempdir().unwrap();
        let url = format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("accounts.db").display()
        );
        let a = SqliteAccounts::connect(&url).await.unwrap();
        let b = SqliteAccounts::connect(&url).await.unwrap();
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
    async fn schema_constraints_and_ledger_are_checked_on_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let url = format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("accounts.db").display()
        );
        let store = SqliteAccounts::connect(&url).await.unwrap();
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
            SqliteAccounts::connect(&url).await,
            Err(Error::Corrupt)
        ));
    }
}
