CREATE TABLE account_schema_version (version INTEGER NOT NULL PRIMARY KEY CHECK (version = 1));
CREATE TABLE accounts (id TEXT PRIMARY KEY NOT NULL, display_name TEXT NOT NULL, created_at BIGINT NOT NULL);
CREATE TABLE external_identities (issuer TEXT NOT NULL, subject TEXT NOT NULL, user_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE, PRIMARY KEY (issuer, subject));
CREATE TABLE login_attempts (state_hash TEXT PRIMARY KEY NOT NULL, browser_hash TEXT NOT NULL, nonce TEXT NOT NULL, pkce_verifier TEXT NOT NULL, expires_at BIGINT NOT NULL);
CREATE TABLE sessions (token_hash TEXT PRIMARY KEY NOT NULL, user_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE, expires_at BIGINT NOT NULL);
CREATE INDEX sessions_expiry ON sessions(expires_at);
CREATE INDEX attempts_expiry ON login_attempts(expires_at);
INSERT INTO account_schema_version VALUES (1);
