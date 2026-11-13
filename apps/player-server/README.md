# player-server

Run the anonymous HTTP API for completed public game records. The service uses
`player-http` with the read-only `PostgresGameRecords` adapter. It needs an
initialized game database and SELECT-only credentials.

See the [HTTP API reference](../../crates/player-http/README.md) for filters,
responses, pagination, and errors.

## Run locally

Start `game-server` first so it initializes the game schema. Then run the reader
from the repository root.

```sh
cargo run --locked -p player-server
```

The server reports `player-server listening on 127.0.0.1:8081`. Use another
terminal to query a completed game. Replace the period with the game's UTC
microsecond interval and use its game ID.

```sh
curl 'http://127.0.0.1:8081/game-records?from=1793577600000000&to=1793664000000000&limit=30'
curl 'http://127.0.0.1:8081/game-records/example-game'
```

## Configuration

| Environment variable | Default | Meaning |
|---|---|---|
| `PLAYER_BIND_ADDR` | `127.0.0.1:8081` | HTTP socket address |
| `PLAYER_GAME_DATABASE_URL` | `postgres://shogi_reader:shogi_reader@localhost:5432/shogi` | PostgreSQL URL with SELECT-only credentials |

The reader requires schema version 6 and never initializes or upgrades the
schema. Its role needs SELECT on `schema_version`, `games`, `journal` and
`rating_states`, without table ownership or write privileges. The [bootstrap SQL](../../scripts/init-postgres.sql) grants access to
new tables created by the game writer.

## Run with Docker Compose

```sh
docker compose --profile records up --build -d
```

Compose connects the reader to the writer's game database with separate
credentials. It publishes HTTP on host loopback, at port 8081 by default.
Set `PLAYER_SERVER_PORT` to change the host port. The restart policy retries
startup while the writer initializes the schema.

## Verification

From the repository root, run the reader and HTTP adapter tests.

```sh
./scripts/test-postgres.sh cargo test --locked -p game-record-persistence -p player-http -p player-server
```

The [API reference](../../crates/player-http/README.md#openapi) describes the
OpenAPI endpoint and export command.

`GET /users/{user_id}/ratings` returns both latest committed rating pools without
authentication. See the [HTTP rating contract](../../crates/player-http/README.md#read-user-ratings).
The reader has no permission to read `rated_games`, `rating_slots`, `rating_jobs`
or `rating_applications`.
