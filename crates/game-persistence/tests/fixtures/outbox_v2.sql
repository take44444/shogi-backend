CREATE TABLE outbox_events (
    id INTEGER PRIMARY KEY, game_id TEXT NOT NULL, seq BIGINT NOT NULL,
    event_kind INTEGER NOT NULL CHECK(event_kind IN (0,1)), encoding_version INTEGER NOT NULL,
    request_id TEXT NOT NULL, user_id TEXT NOT NULL, usi TEXT, in_check BOOLEAN,
    winner INTEGER, reason INTEGER,
    published BOOLEAN NOT NULL DEFAULT 0 CHECK(published IN (0,1)),
    UNIQUE(game_id, seq, event_kind),
    FOREIGN KEY(game_id, seq) REFERENCES journal(game_id, seq),
    CHECK((winner IS NULL AND reason IS NULL) OR
        (winner IS NOT NULL AND reason IS NOT NULL AND winner IN (1,2) AND reason IN (1,2))),
    CHECK((event_kind = 0 AND usi IS NOT NULL AND in_check IS NOT NULL
            AND in_check IN (0,1) AND (reason IS NULL OR reason = 1))
        OR (event_kind = 1 AND usi IS NULL AND in_check IS NULL AND winner IS NOT NULL))
);
CREATE INDEX outbox_pending ON outbox_events(published, id);
UPDATE schema_version SET version = 2;
