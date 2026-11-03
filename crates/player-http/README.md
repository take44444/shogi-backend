# player-http

`router(GameRecords)` exposes anonymous read access to completed public records.
It accepts no account credentials and has no user database connection.

The routes are `GET /game-records` and `GET /game-records/{id}`. The adapter parses
HTTP input into application types and maps public values into explicit JSON DTOs.
A list returns `{ "items": [...], "next_cursor": null }`. Detail returns
`{ "metadata": {...}, "history": [...] }`. History operations contain `seq`,
`player`, and `action`. Moves also contain canonical `usi`. Resignation has no USI.

All handled responses use `Cache-Control: no-store`. Invalid query fields return
400 JSON. Missing, ongoing, and hidden records return identical 404 JSON.
Corruption returns 500, storage failure or full request capacity returns 503,
and an oversized history returns 422. Error bodies contain only an `error` code.
The adapter accepts up to 16 active requests. Serialized responses are limited
to 1 MiB. SQL acquisition and execution limits belong to the repository.

See [player-server](../../apps/player-server/README.md) for complete query and
configuration reference. Router tests exercise a real initialized SQLite database,
public JSON fields, invalid inputs, indistinguishable missing records, and errors.
