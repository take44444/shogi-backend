# player-http

`router(GameRecords, UserRatings)` exposes anonymous HTTP access to completed public game
records. The adapter parses queries into application types and returns JSON.
It uses no account credentials or user database connection.

See the [player-server guide](../../apps/player-server/README.md) for startup,
environment variables, and database permissions.

## Search completed games

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
appear before an earlier page. Results are not fixed across requests. Cursor
fields remain untrusted and are not signed credentials.

## Read one game's history

`GET /game-records/{id}` returns one metadata object and an ordered `history`.

A move is `{ "seq": 1, "player": "black", "action": "move", "usi": "7g7f" }`.
Resignation is `{ "seq": 2, "player": "white", "action": "resign" }`.
The history comes from the same database snapshot as metadata. It has at most
4096 accepted operations, with contiguous sequences and consistent terminal
results and move count. This is a detail-read limit, not a limit on game commands.
The reader validates syntax and structural consistency without replaying move
legality.

## Errors and resource limits

Only completed public games are readable. Hidden, ongoing, and nonexistent
games return the same 404 response.

| HTTP status | Error code | Meaning |
|---|---|---|
| 400 | `invalid_query` | Invalid or missing filter, ID, range, limit, or cursor |
| 404 | `not_found` | No completed public record available |
| 422 | `history_too_long` | More than 4096 operations or a response over 1 MiB |
| 500 | `invalid_record` | Corrupt stored record, without partial history |
| 503 | `unavailable` | Storage failure, statement deadline, pool timeout, or full request capacity |

Error responses use `{ "error": "code" }`. Handled responses have
`Cache-Control: no-store`. The HTTP adapter permits 16 active queries, and the
PostgreSQL reader has four connections. Pool acquisition and PostgreSQL statement
execution each have a two-second bound. Connections default to read-only
transactions. Database role grants prevent table writes even if a session changes
that default. Game record responses exclude request IDs, receipts, expected
sequences, and outbox data.

## OpenAPI

`GET /openapi.json` serves the OpenAPI 3.1 document. `player_http::openapi()`
constructs the same document without a database or service configuration.
Export it from the repository root.

```sh
cargo run --locked -p player-http --example player-openapi
```

Handler annotations and serialized DTOs define the schema. `api_router()` uses
`OpenApiRouter` to register both runtime routes and documented paths. Register
new handlers with `routes!` and keep `#[utoipa::path]` beside each handler.
The [root guide](../../README.md#generate-api-documentation) covers exporting
both services' documents.

## Verification

Run adapter tests against disposable PostgreSQL from the repository root.

```sh
./scripts/test-postgres.sh cargo test --locked -p player-http
```

Tests cover JSON fields, invalid queries, missing and hidden records, resource
limits, and the generated OpenAPI contract. [Rust CI](../../.github/workflows/rust.yaml)
runs these tests with the workspace tests.

## Read user ratings

`GET /users/{user_id}/ratings` requires no authentication. The response always
contains `ten_minute_30` and `byoyomi_30`. Each pool contains `rating`, an integer
or null, and `completed_rated_games`, a nonnegative integer.

```json
{"ten_minute_30":{"rating":1516,"completed_rated_games":1},"byoyomi_30":{"rating":null,"completed_rated_games":0}}
```

Null and zero mean no applied rated games. A syntactically valid unknown user
has the same response. The endpoint does not confirm account existence or that
pending games have been applied. It reads the latest committed state.

Invalid IDs return 400. Storage failures and exhausted request capacity return
503. Responses include `Cache-Control: no-store`. The route and nullable rating
schema appear in `/openapi.json` and the offline `player-openapi` example.
