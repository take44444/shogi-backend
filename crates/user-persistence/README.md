# user-persistence

`PostgresAccounts` implements `AccountRepository` through SeaORM. It owns account
storage in a separate PostgreSQL database: profiles, external identities, login
attempts, sessions, and a schema version ledger. It has no game dependency.

## Schema startup

Startup initializes an empty schema or validates the existing versioned schema.
The database and role must already exist. The role needs database `CREATE`
permission and permission to create objects in `public`. Existing-schema
validation creates a reference schema inside a transaction and drops it before
commit. Database `CREATE` permission is therefore required on every startup.
Superuser and `CREATEROLE` privileges are not required by this adapter.

Data operations, table definitions, and index definitions use SeaQuery builders.
PostgreSQL advisory locks, catalog inspection, and reference-schema management
use handwritten SQL.

## Login and sessions

One transaction resolves the unique Google issuer and subject, creates an internal UUID when needed, and stores the new session. The unique identity constraint and transaction synchronization make concurrent first logins converge on one account. Sessions have an absolute expiry and store only the token hash. Logout deletes the session row.

Login attempts retain the nonce and PKCE verifier for ten minutes. A single `DELETE ... RETURNING` consumes an unexpired attempt only when its state hash and browser cookie hash match. Expired attempts and sessions are pruned when a login starts. Protect the database and its backups as account data.

## Verification

Contract tests use isolated PostgreSQL databases through `pg-test-support`.
Run them from the repository root.

```sh
./scripts/test-postgres.sh cargo test --locked -p user-persistence
```
