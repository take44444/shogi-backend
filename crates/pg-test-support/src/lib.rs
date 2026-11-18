mod games;
pub use games::{GameFixture, rated_identity, seed_game, seed_rated};

use sqlx::{Connection, PgConnection};

/// A disposable database owned by one test. Requires a role with CREATEDB and CREATEROLE.
pub struct TestDatabase {
    admin_url: String,
    url: String,
    name: String,
    reader: String,
}
impl TestDatabase {
    /// Creates an empty database using TEST_DATABASE_URL. No test silently skips storage.
    pub async fn new() -> Self {
        let admin_url = std::env::var("TEST_DATABASE_URL").expect(
            "set TEST_DATABASE_URL to a PostgreSQL administration URL or run scripts/test-postgres.sh",
        );
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let name = format!("shogi_test_{suffix}");
        let reader = format!("shogi_reader_{suffix}");
        let mut admin = PgConnection::connect(&admin_url)
            .await
            .expect("connect to test PostgreSQL");
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
            .execute(&mut admin)
            .await
            .expect("create isolated database");
        let mut url = url::Url::parse(&admin_url).expect("PostgreSQL URL");
        url.set_path(&format!("/{name}"));
        Self {
            admin_url,
            url: url.to_string(),
            name,
            reader,
        }
    }
    /// Returns the isolated writer database URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Creates a database role limited to the explicitly public read tables.
    pub async fn reader_url(&self) -> String {
        let mut admin = PgConnection::connect(&self.admin_url).await.unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "CREATE ROLE {} LOGIN PASSWORD '{}'",
            self.reader, self.reader
        )))
        .execute(&mut admin)
        .await
        .unwrap();
        let mut connection = PgConnection::connect(&self.url).await.unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("GRANT USAGE ON SCHEMA public TO {}; GRANT SELECT ON public.schema_version, public.games, public.journal, public.rating_states TO {}; ALTER ROLE {} SET default_transaction_read_only = on; ALTER ROLE {} SET statement_timeout = '2s'; ALTER ROLE {} SET lock_timeout = '2s'", self.reader, self.reader, self.reader, self.reader, self.reader))).execute(&mut connection).await.unwrap();
        let mut url = url::Url::parse(&self.url).unwrap();
        url.set_username(&self.reader).unwrap();
        url.set_password(Some(&self.reader)).unwrap();
        url.to_string()
    }
}
impl Drop for TestDatabase {
    fn drop(&mut self) {
        let admin_url = self.admin_url.clone();
        let name = self.name.clone();
        let reader = self.reader.clone();
        let cleanup = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let mut admin = PgConnection::connect(&admin_url).await?;
                    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                        "DROP DATABASE {name} WITH (FORCE)"
                    )))
                    .execute(&mut admin)
                    .await?;
                    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP ROLE IF EXISTS {reader}")))
                        .execute(&mut admin)
                        .await?;
                    Ok::<_, sqlx::Error>(())
                })
        })
        .join();
        if !matches!(cleanup, Ok(Ok(()))) {
            eprintln!(
                "failed to clean up isolated PostgreSQL test database {}",
                self.name
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    #[tokio::test]
    async fn test_databases_have_independent_names_and_contents() {
        let first = TestDatabase::new().await;
        let second = TestDatabase::new().await;
        assert_ne!(first.url(), second.url());
        let mut left = PgConnection::connect(first.url()).await.unwrap();
        sqlx::raw_sql(
            "CREATE TABLE isolation_probe (id INTEGER); INSERT INTO isolation_probe VALUES (1)",
        )
        .execute(&mut left)
        .await
        .unwrap();
        let row = sqlx::query("SELECT count(*) AS n FROM isolation_probe")
            .fetch_one(&mut left)
            .await
            .unwrap();
        assert_eq!(row.get::<i64, _>("n"), 1);
        let mut right = PgConnection::connect(second.url()).await.unwrap();
        let row = sqlx::query("SELECT to_regclass('public.isolation_probe')::text AS name")
            .fetch_one(&mut right)
            .await
            .unwrap();
        assert_eq!(row.get::<Option<String>, _>("name"), None);
    }
}
