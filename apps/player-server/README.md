# player-server

The server exposes completed public game records through `player-http` and the
read-only `SqliteGameRecords` repository. Rates, matching, AI, user authentication,
and ongoing-game synchronization are outside this implementation.

Start `game-server` first so it initializes the game database.
Then run `cargo run -p player-server` from the workspace root.

| Environment variable | Default | Meaning |
|---|---|---|
| `PLAYER_BIND_ADDR` | `127.0.0.1:8081` | HTTP socket address |
| `PLAYER_GAME_DATABASE_PATH` | `data/shogi.db` | Existing SQLite file path, not a database URL |

`docker compose --profile records up --build -d` starts this service with the game
volume mounted read-only. Its restart policy retries startup while the writer
creates the database. It requires schema version 5 and readable live
WAL and SHM files. It never initializes or creates the database.

## Query reference

`GET /game-records` requires both `from` and `to`. The interval uses UTC integer
microseconds, includes `from`, excludes `to`, and spans at most 366 days.
All filters combine with AND. Unknown or repeated query fields return 400.

| Parameter | Values and meaning |
|---|---|
| `participant` | Valid user ID, matched against either color |
| `rule` | `standard` |
| `winner` | `black` or `white`, independent of the participant filter |
| `reason` | `checkmate` or `resignation` |
| `from`, `to` | Required nonnegative UTC microseconds with `from < to` |
| `min_ply`, `max_ply` | Optional inclusive final move counts from 0 to `i64::MAX`, excluding resignation |
| `limit` | 1 through 100, default 30 |
| `cursor` | Opaque next cursor returned by the preceding page |

This example covers 2026-11-02 UTC. Replace the interval for another date.

```sh
curl 'http://127.0.0.1:8081/game-records?from=1793577600000000&to=1793664000000000&participant=alice&rule=standard&winner=black&reason=resignation&min_ply=0&max_ply=200&limit=30'
curl 'http://127.0.0.1:8081/game-records/example-game'
```

The list response contains metadata without history or totals. Metadata fields
are `id`, `black`, `white`, `rule`, `result`, `finished_at`, `final_seq`, and
`final_move_ply`. `result` contains `winner` and `reason`. `finished_at` is the
writer's commit-preparation UTC timestamp. `final_seq` counts accepted moves and
resignation. `final_move_ply` counts moves only.

Results sort by descending completion time, then descending binary game ID.
Pass `next_cursor` unchanged while retaining the filters. The page limit can
change. Each page rechecks public visibility and filters. New completions may
appear before an earlier page, so pagination does not freeze a cross-request
snapshot. Cursor fields remain untrusted and are not signed credentials.

`GET /game-records/{id}` returns one metadata object and an ordered `history`.
A move is `{ "seq": 1, "player": "black", "action": "move", "usi": "7g7f" }`.
Resignation is `{ "seq": 2, "player": "white", "action": "resign" }`.
The history comes from the same database snapshot as metadata. It has at most
4096 accepted operations, with contiguous sequences and consistent terminal
results and move count. The reader validates syntax and structural consistency,
without replaying move legality.

## Publication and errors

New games use explicit standard rules and public visibility. Their records become
readable only after completion. Existing databases must already use schema version 5.
Incompatible formats fail startup without changes. A hidden, ongoing, or nonexistent
record returns the same response.

| HTTP status | Error code | Meaning |
|---|---|---|
| 400 | `invalid_query` | Invalid or missing filter, ID, range, limit, or cursor |
| 404 | `not_found` | No completed public record available |
| 422 | `history_too_long` | More than 4096 operations or a response over 1 MiB |
| 500 | `invalid_record` | Corrupt stored record, without partial history |
| 503 | `unavailable` | Storage failure, SQLite deadline, pool timeout, or full request capacity |

Error responses use `{ "error": "code" }`. Handled responses have
`Cache-Control: no-store`. The HTTP adapter permits 16 active queries, and the
SQLite reader has four connections. Acquisition, busy waits, and SQLite execution
each have a two-second bound. SQLite progress handlers interrupt long queries.
No response includes retry IDs, receipts, expected sequences, or outbox data.

`GET /openapi.json` serves the OpenAPI 3.1 contract. To export it without server
startup or configuration, run `cargo run -p player-http --example player-openapi`.
See the [HTTP adapter reference](../../crates/player-http/README.md) for generation
and contract checks.
