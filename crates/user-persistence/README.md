# user-persistence

`PostgresAccounts` implements `AccountRepository` through SeaORM. It owns a separate PostgreSQL database with its own version ledger, accounts, external identities, login attempts, and sessions. Startup initializes an empty database or validates its versioned schema. The database and role must already exist. The role must own the database, or have `CREATE ON DATABASE` and `CREATE ON SCHEMA public`. Existing-schema validation builds a reference schema inside a transaction and drops it before commit. This requires database CREATE permission on every startup; it does not require a superuser or CREATEROLE.

One transaction resolves the unique Google issuer and subject, creates an internal UUID when needed, and stores the new session. The unique identity constraint and transaction synchronization make concurrent first logins converge on one account. Sessions have an absolute expiry and store only the token hash. Logout deletes the session row.

Login attempts retain the nonce and PKCE verifier for ten minutes. A single `DELETE ... RETURNING` consumes an unexpired attempt only when its state hash and browser cookie hash match. Expired attempts and sessions are pruned when a login starts. Protect the database and its backups as account data.

The crate does not depend on game code or initialize game databases. Contract tests use isolated PostgreSQL databases through `pg-test-support`. Run them with `./scripts/test-postgres.sh cargo test --locked -p user-persistence` from the workspace root.
