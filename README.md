# Shogi backend

A shogi backend with durable gRPC game commands, public HTTP game records, and
Google login through a separate HTTP user service. PostgreSQL stores accepted
commands and events. An independent publisher sends transient notifications
through Valkey Pub/Sub.

The realtime gateway exposes authenticated browser WebSockets for moves,
resignation, opponent events, and signed-in spectators. See the
[gateway guide](apps/realtime-gateway/README.md) for its browser contract and
same-origin setup.

Game gRPC trusts caller-supplied user IDs. Keep it on a trusted internal network.
The user service does not authenticate game gRPC requests.

Use Rust, Cargo, Docker, and Docker Compose for the commands below. Run them from
the repository root. The build supplies a vendored `protoc`.

## Run a local game

Start PostgreSQL, then start the game server.

```sh
docker compose up -d --wait postgres
cargo run --locked -p game-server
```

The server reports `game-server listening on 127.0.0.1:50051`. In another
terminal, run the example client.

```sh
cargo run --locked -p game-grpc --example play
```

The client creates a game, plays `7g7f 3c3d 8h2b+ 3a2b B*4e`, and resigns White.
It checks capture, promotion, drops, snapshots, and exact retries. The final line
reports `black wins at sequence 6`. Each run generates a new game ID unless you
supply one.

The server defaults to `127.0.0.1:50051` and
`DATABASE_URL=postgres://shogi:shogi@localhost:5432/shogi`. A positional address
overrides `GAME_SERVER_ADDR`.

```sh
DATABASE_URL='postgres://shogi:shogi@localhost:5432/shogi' cargo run --locked -p game-server -- 127.0.0.1:50052
cargo run --locked -p game-grpc --example play -- http://127.0.0.1:50052 my-game
```

Startup initializes or validates schema version 6 before listening. The database
and role must already exist. Schema validation requires database `CREATE`
permission even on subsequent startups. See the
[storage reference](crates/game-persistence/README.md#schema-startup) for required
permissions and incompatible-schema handling.

## Read public completed game records

After the game server initializes the database, start the record service.

```sh
cargo run --locked -p player-server
```

Query it from another terminal. Replace the period with the game's UTC interval,
expressed in microseconds, and use the ID of a completed game.

```sh
curl 'http://127.0.0.1:8081/game-records?from=1793577600000000&to=1793664000000000&limit=30'
curl 'http://127.0.0.1:8081/game-records/my-game'
```

The service uses separate SELECT-only credentials and needs no user service.
See [player-server](apps/player-server/README.md) for startup and
[player-http](crates/player-http/README.md) for query parameters, responses, and
limits. For Google login and profile operations, follow the
[user-server guide](apps/user-server/README.md).

## Run with Docker Compose

Start the game server, publisher, Valkey, and PostgreSQL.

```sh
docker compose up --build -d
```

The PostgreSQL bootstrap creates the `shogi` and `users` databases and the
`shogi_reader` role. Both databases use the `pg-data` named volume. Published
ports bind to host loopback. Set `GAME_SERVER_PORT`, `POSTGRES_PORT`, or
`VALKEY_PORT` to override their host ports.

Add the public record service with the `records` profile.

```sh
docker compose --profile records up --build -d
```

The optional `auth` profile runs the user service. Configure Google credentials
as described in the [user-server guide](apps/user-server/README.md#run-with-docker-compose).
The `realtime` profile adds a same-origin browser edge, gateway, and a User Service
configured for that edge. Run `docker compose --profile realtime up --build -d`
and use `http://localhost:8082` for login and game WebSockets. Set Google
credentials before logging in. The gateway receives no database URL.
`docker compose down` stops services and retains database data.

## Prove persistence across container recreation

Start Compose as described above. Use a fresh game ID and proof directory.
Before each client invocation, wait for
`game-server listening on 0.0.0.0:50051` in `docker compose logs game-server`.
These commands assume the default gRPC port.

```sh
cargo run --locked -p game-grpc --example persistence_proof -- seed http://127.0.0.1:50051 restart-proof /tmp/shogi-proof
docker compose up -d --force-recreate game-server
cargo run --locked -p game-grpc --example persistence_proof -- resume http://127.0.0.1:50051 restart-proof /tmp/shogi-proof
docker compose restart game-server
cargo run --locked -p game-grpc --example persistence_proof -- finished http://127.0.0.1:50051 restart-proof /tmp/shogi-proof
```

The client saves protobuf snapshots and receipts, compares them after restart,
continues play, and checks original receipts after restarting the finished game.
The same commands work with a local server if you restart it between phases
with the same `DATABASE_URL`.

## Publish committed game events

The game server commits accepted commands and their events atomically. It
accepts commands while the publisher or broker is unavailable. To run a local
publisher against the same database, start Valkey first.

```sh
docker compose up -d --wait valkey
DATABASE_URL='postgres://shogi:shogi@localhost:5432/shogi' VALKEY_URL='redis://127.0.0.1:6379' cargo run --locked -p outbox-publisher
```

Stop any Compose publisher for that database before starting a local one.
Only one publisher may own the database's outbox at a time.

Events use `shogi.games.<hex-game-id>.events`, where the game ID is encoded as
lowercase hexadecimal UTF-8 bytes. Notification acceptance does not guarantee
subscriber delivery or processing. Disconnected subscribers miss notifications,
and uncertain sends can produce duplicates. Consumers must deduplicate event
identities and recover missing state. See the
[application contract](crates/game-application/README.md#event-publication),
[publisher ownership reference](crates/game-persistence/README.md#outbox-ownership),
and [event delivery decision](docs/adr/0003-game-event-delivery.md).

The JSON envelope contains `schema_version: 1`, `event_id`, `type`, `game_id`,
`seq`, `request_id`, and `user_id`. Move notifications also contain canonical
USI `movement`, `in_check`, and `result`. The result is null while the game is
ongoing, or contains `winner` and `reason`. Event types are `move_committed` and
`game_finished`; winners are `black` and `white`; reasons are `checkmate` and
`resignation`. The event ID joins the hexadecimal game ID, sequence, and event
type with colons. A mating move emits both event types at the same sequence.
Version-one encoder changes must preserve this mapping.

Failed or timed-out publication leaves the event pending and stops the worker.
Compose restarts it to retry. Published rows record broker acceptance only and
remain stored; they do not represent a consumer's completion.

Run the end-to-end proof with a disposable Compose project.

```sh
./scripts/prove-outbox.sh
```

The proof checks notifications, exact retries, publisher downtime, server
recreation, Valkey outages, and recovery. It also checks notification acceptance
with no subscribers. Default host ports are 55051, 56379, and 55435; override them
with `GAME_SERVER_PORT`, `VALKEY_PORT`, and `POSTGRES_PORT`. Cleanup removes the
proof project's containers and database volume.

## Generate API documentation

Export OpenAPI 3.1 JSON for both HTTP services without starting servers or
configuring a database or Google credentials.

```sh
bash scripts/generate-openapi.sh
```

The output is `target/openapi/player.json` and `target/openapi/user.json`.
An optional first argument selects another directory. Running HTTP services also
serve their document at `GET /openapi.json`.

Generate Rust API documentation with rustdoc.

```sh
cargo doc --locked --workspace --no-deps
cargo doc --locked -p game-application --no-deps --open
```

HTML is written under `target/doc/`. Add `--document-private-items` to inspect
internal modules. Protocol comments from `proto/game.proto` appear in the
generated `game_grpc::proto` reference.

## Verify the workspace

Run the checks used by [Rust CI](.github/workflows/rust.yaml).

```sh
python3 scripts/check-boundaries.py
python3 scripts/test-check-boundaries.py
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
./scripts/test-postgres.sh
python3 scripts/check-contract-interfaces.py
python3 scripts/test-deployment.py
```

The boundary checker inspects workspace dependency paths and direct external
dependency declarations. Its tests verify that forbidden configurations fail.
The interface checker compiles the examples in `tests/contracts/`.
The deployment check starts disposable PostgreSQL and probes actual permissions.

`test-postgres.sh` starts isolated PostgreSQL and Valkey Compose services and
removes them after the tests. Their default host ports are 55433 and 56333;
override them with `POSTGRES_PORT` and `VALKEY_PORT`. If `TEST_DATABASE_URL` or
`TEST_VALKEY_URL` is set, the script uses that service instead and starts only
the missing one. The database URL must permit `CREATEDB` and `CREATEROLE`.
Each database test creates its own database and removes it afterward.

To run a package's database tests or check Rust documentation, use:

```sh
./scripts/test-postgres.sh cargo test --locked -p game-persistence
RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps
cargo test --locked --workspace --doc
```

Compose credentials are local development defaults. Production deployments
supply PostgreSQL credentials, TLS configuration, backups, and restricted roles.
Apply the reader grants from the [bootstrap SQL](scripts/init-postgres.sql) for
tables owned by the game writer. The application creates neither databases nor
roles.

The [documentation index](docs/README.md) links to adopted decisions, unfinished
work, and future plans. Crate references describe the
[domain rules](crates/shogi-domain/README.md),
[application contract](crates/game-application/README.md),
[storage](crates/game-persistence/README.md), and
[gRPC API](crates/game-grpc/README.md).

## Rated games

`CreateGame` remains casual. Trusted game callers use `CreateRatedGame` with
`TEN_MINUTE_30` for 600 seconds plus 30-second byoyomi, or `BYOYOMI_30` for zero
main time plus 30-second byoyomi. These choices do not run clocks.

The rating worker starts with the default Compose services. The public player
server returns the latest committed values at `GET /users/{user_id}/ratings`.
See [rating behavior](docs/design/ratings.md) and [running the rating worker](apps/rating-worker-server/README.md).

Schema version 6 supports fresh databases only. Startup rejects previous schemas
without migration or data deletion. Keep any existing database and provision a
separate empty database for this version.
