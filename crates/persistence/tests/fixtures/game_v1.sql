CREATE TABLE schema_version (id INTEGER PRIMARY KEY NOT NULL, version INTEGER NOT NULL);
INSERT INTO schema_version VALUES (0, 1);
CREATE TABLE users (id TEXT PRIMARY KEY NOT NULL);
CREATE TABLE games (
    id TEXT PRIMARY KEY NOT NULL, black TEXT NOT NULL REFERENCES users(id), white TEXT NOT NULL REFERENCES users(id),
    version BIGINT NOT NULL CHECK(version >= 0), winner INTEGER, reason INTEGER,
    CHECK(black != white),
    CHECK((winner IS NULL AND reason IS NULL) OR
        (winner IS NOT NULL AND reason IS NOT NULL AND winner IN (1,2) AND reason IN (1,2)))
);
CREATE TABLE journal (
    game_id TEXT NOT NULL REFERENCES games(id), seq BIGINT NOT NULL CHECK(seq > 0),
    request_id TEXT NOT NULL, user_id TEXT NOT NULL REFERENCES users(id),
    expected_seq BIGINT NOT NULL CHECK(expected_seq >= 0), action INTEGER NOT NULL,
    usi TEXT, in_check BOOLEAN NOT NULL, winner INTEGER, reason INTEGER,
    PRIMARY KEY(game_id, seq), UNIQUE(game_id, request_id),
    CHECK(expected_seq = seq - 1),
    CHECK((action = 1 AND usi IS NOT NULL) OR (action = 2 AND usi IS NULL)),
    CHECK((winner IS NULL AND reason IS NULL) OR
        (winner IS NOT NULL AND reason IS NOT NULL AND winner IN (1,2) AND reason IN (1,2)))
);
