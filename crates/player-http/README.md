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

`GET /openapi.json` returns this service's OpenAPI 3.1 document. The public
`player_http::openapi()` constructor needs no database, service configuration, or
provider connection. Handler annotations, serde DTOs, and one `OpenApiRouter`
registry define both the runtime routes and the document. Register new handlers
with `routes!` in `api_router()` and keep their `#[utoipa::path]` annotations
next to the handlers. No separate document path list is maintained.

Export this service's document without starting the server.

```sh
cargo run -p player-http --example player-openapi
```

From the repository root, `bash scripts/generate-openapi.sh` writes both service
documents to `target/openapi/player.json` and `target/openapi/user.json`. Pass an
output directory as its first argument to choose another destination. Generated
JSON is a build artifact. CI runs HTTP contract tests, generates both documents,
and validates them with `openapi-spec-validator`.
