# User persistence

`SqliteAccounts` implements `AccountRepository` through SeaORM. It owns a separate account database with its own version ledger, accounts, external identities, login attempts, and sessions. Opening an existing database compares every table and index definition with the versioned schema and rejects incompatible databases.

An immediate SQLite transaction resolves the unique Google issuer and subject, creates an internal UUID when needed, and stores the new session. Concurrent first logins converge on one account. Sessions have an absolute expiry and store only the token hash. Logout deletes the session row.

Login attempts retain the nonce and PKCE verifier for ten minutes. A single `DELETE ... RETURNING` consumes an unexpired attempt only when its state hash and browser cookie hash match. Expired attempts and sessions are pruned when a login starts. Protect the SQLite file and its backups as account data.

The crate does not depend on game code or migrate game databases.
