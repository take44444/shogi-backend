# Shogi backend

Run a durable shogi game over gRPC with PostgreSQL and a generated Rust client.
Committed moves and game results are stored in a transactional outbox. An
independent publisher sends them to Core NATS.
Rust, Cargo, and Docker Compose are required for these local commands. Cargo supplies vendored protoc.
Start PostgreSQL before the local server. The bootstrap creates separate `shogi` and `users` databases and a read-only `shogi_reader` role.

```sh
docker compose up -d --wait postgres
```

Start the server from the repository root.

```sh
cargo run -p game-server
```

In another terminal, run the client.

```sh
cargo run -p game-grpc --example play
```

The client binds two example participant IDs to a game, plays `7g7f 3c3d 8h2b+ 3a2b B*4e`,
and resigns White. It verifies capture, promotion, recapture, a bishop drop,
snapshots, and exact retries. The final line reports `black wins at sequence 6`.
Each invocation needs a new game ID. Omitting the ID generates one.

The server defaults to `127.0.0.1:50051` and
`DATABASE_URL=postgres://shogi:shogi@localhost:5432/shogi`. Startup initializes or validates the schema in the existing database before listening. Participant
bindings, positions, results, and accepted request receipts survive restart.
A positional address overrides `GAME_SERVER_ADDR`.

```sh
DATABASE_URL='postgres://shogi:shogi@localhost:5432/shogi' cargo run -p game-server -- 127.0.0.1:50052
cargo run -p game-grpc --example play -- http://127.0.0.1:50052 my-game
```

## Read public completed game records

After `game-server` initializes the database, run the anonymous record API.

```sh
cargo run -p player-server
curl 'http://127.0.0.1:8081/game-records?from=1793577600000000&to=1793664000000000&limit=30'
curl 'http://127.0.0.1:8081/game-records/my-game'
```

Replace the period with the game's UTC microsecond interval. Lists contain only
metadata. Detail includes ordered moves and resignation. New games are public
once finished. No user service is required.
The reader connects to the existing game database with the `shogi_reader` role. PostgreSQL grants this role SELECT access and no table write privileges.
See [player-server](apps/player-server/README.md) for filters, limits, and errors.
`docker compose --profile records up --build -d` also starts the reader with its own database credentials.

## Prove persistence across container recreation

Build and start the services. Compose runs `game-server`, `outbox-publisher`, and
NATS, and PostgreSQL. The server and publisher connect to the `shogi` database. PostgreSQL stores both service databases in the `pg-data` named volume.
Compose binds the gRPC port to the host loopback address. Set `GAME_SERVER_PORT` to
change the host port. These commands assume the default port.

```sh
docker compose up --build -d
cargo run -p game-grpc --example persistence_proof -- seed http://127.0.0.1:50051 restart-proof /tmp/shogi-proof
docker compose up -d --force-recreate game-server
cargo run -p game-grpc --example persistence_proof -- resume http://127.0.0.1:50051 restart-proof /tmp/shogi-proof
docker compose restart game-server
cargo run -p game-grpc --example persistence_proof -- finished http://127.0.0.1:50051 restart-proof /tmp/shogi-proof
```

Wait for `game-server listening on 0.0.0.0:50051` in
`docker compose logs game-server` before each client invocation. Use a fresh game
ID and proof directory for another run. The client stores protobuf snapshots and
receipts in the proof directory, compares them after restart, continues play,
and verifies both original receipts after a second restart of the finished game.

`docker compose down` stops the services while retaining the database volume.
The same seed, resume, and finished commands also work with a local server.
Stop and restart the local server between phases with the same `DATABASE_URL`.

## Publish committed game events

The game actor generates and freezes events in a `StagedGameChange` before
calling the repository. The repository commits game metadata, the supplied receipt,
and those events in one PostgreSQL transaction. A move produces `MoveCommitted`; resignation
produces `GameFinished`. A mating move produces both at the same sequence.
Exact command retries return their original acceptance without adding events.
The game server accepts commands while the publisher or broker is unavailable.

The publisher subscribes to the event's subject, sends the pending event, and
waits for an echo with the exact payload on the same connection before recording
it as published. It needs publish and subscribe access to game event subjects.
A socket flush alone does not confirm server processing. Failed operations exit the worker,
and Compose restarts it to retry pending rows. Transparent reconnection is
disabled so an echo on a replacement connection cannot confirm an earlier send.
A failed or uncertain operation can resend the same event.
Consumers deduplicate the full event identity and use a game snapshot
to recover missing sequences. Core NATS does not retain events for disconnected
subscribers, and the publisher's echo does not acknowledge other subscribers' delivery.
The echo confirms a matching event, rather than acknowledging a particular
client's publish operation.

The PostgreSQL publisher supports one active process per game database. A dedicated connection holds a session advisory lock, so a second publisher fails startup. All queue reads and acknowledgements use that connection. Losing the connection stops publication. Dispatch holds no database transaction across network calls. Published rows remain stored, and the first pending failure blocks later events.

For a local publisher, start NATS and use the same database URL as the server.

```sh
DATABASE_URL='postgres://shogi:shogi@localhost:5432/shogi' NATS_URL='nats://127.0.0.1:4222' cargo run -p outbox-publisher
```

Events use the subject `shogi.games.<hex-game-id>.events`, where `<hex-game-id>`
is lowercase hexadecimal of the game ID's UTF-8 bytes. This preserves IDs that
contain dots, NATS wildcards, or Unicode. The JSON envelope has `schema_version`
1, a stable `event_id`, `type`, original `game_id`, `seq`, `request_id`, and
`user_id`. Move events also contain canonical USI `movement` and `in_check`.
`result` is null for an ongoing game or contains `winner` and `reason`.
Wire types are `move_committed` and `game_finished`; winners are `black` and
`white`; finish reasons are `checkmate` and `resignation`. The event ID combines
the hexadecimal game ID, sequence, and event type with colons.
Pending rows preserve typed event fields rather than serialized JSON bytes.
Future encoder changes must preserve this version-one wire mapping.

New game databases initialize schema version 5 atomically. Existing databases must
already have the current version ledger and required tables. Startup validates
that format without changing game records, receipts, or outbox publication state.
Incompatible or incomplete databases fail startup without upgrading or deleting data.

Run the Compose proof from the repository root.

```sh
./scripts/prove-outbox.sh
```

It creates an isolated Compose project with a disposable PostgreSQL database volume. It
checks actual NATS payloads, exact retries, backlog after publisher downtime,
server recreation, commands during a NATS outage, and recovery without
republishing marked rows. It uses host ports 55051, 54222, and 55435 unless overridden
by `GAME_SERVER_PORT`, `NATS_PORT`, and `POSTGRES_PORT`. Cleanup removes only that proof project's
containers and volume.

## Read the API documentation

Generate OpenAPI 3.1 JSON for both HTTP services without a database, credentials,
or running servers.

```sh
bash scripts/generate-openapi.sh
```

The documents are `target/openapi/player.json` and `target/openapi/user.json`.
An optional first argument selects the output directory. Running services expose
these documents at `GET /openapi.json`. The handler annotations and serialized
HTTP DTOs in [player-http](crates/player-http/README.md) and
[user-http](crates/user-http/README.md) are the schema sources.


Generate the workspace API reference with rustdoc.

```sh
cargo doc --workspace --no-deps
```

The HTML is written under `target/doc/`. Open a crate's reference in your browser.

```sh
cargo doc -p game-application --no-deps --open
```

The crate pages describe domain rules, application commands and errors, the
storage contract, and the gRPC adapter. Protocol message comments originate in
`proto/game.proto` and appear in the generated `game_grpc::proto` reference.

Include private modules when inspecting actor recovery and schema initialization.

```sh
cargo doc --workspace --no-deps --document-private-items
```

Check documentation links and examples before changing the public contract.

```sh
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test --workspace --doc
```

## Verify the workspace

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
./scripts/test-postgres.sh
```

`scripts/test-postgres.sh` starts an isolated PostgreSQL Compose project, waits for readiness, exports `TEST_DATABASE_URL`, and runs the workspace tests. Cleanup removes that project's containers and database volume. Set `POSTGRES_PORT` to override its default host port 55433. To use an existing test server, set `TEST_DATABASE_URL` to an administrative database URL with `CREATEDB` and `CREATEROLE` permissions. Each database test creates a unique database and drops it after the test. Database tests fail if PostgreSQL is unavailable.

```sh
TEST_DATABASE_URL='postgres://postgres:postgres@localhost:5432/postgres' cargo test --locked --workspace
./scripts/test-postgres.sh cargo test --locked -p game-persistence
```

Compose credentials are local development defaults. Production deployments supply their own PostgreSQL credentials, TLS configuration, backups, and restricted database roles. Apply the reader grants from [the bootstrap SQL](scripts/init-postgres.sql) for tables owned by the game writer. The application never creates its database or PostgreSQL roles.

The [application contract](crates/game-application/README.md) covers participant
binding, sequencing, replay, cancellation, and recovery. The
[storage reference](crates/game-persistence/README.md) covers PostgreSQL transactions and
schema startup. The [RPC reference](crates/game-grpc/README.md) covers wire fields and
status codes. The [domain crate](crates/shogi-domain/README.md) owns shogi rules.

User registration, Google login, profile updates, and session management belong
to the independent HTTP user service. See the [user-server README](apps/user-server/README.md)
for configuration, startup, and verification, and the [HTTP API reference](crates/user-http/README.md)
for endpoints.
It uses a separate PostgreSQL database and shares only the `user-id` value type
with game code. User management has no gRPC endpoints.

Game gRPC still accepts caller-supplied identity claims and belongs on a trusted
internal network. The user service does not authenticate game gRPC requests.
Game creation binds two distinct user IDs without consulting the user database.
Matchmaking, shards, leases, game clocks, Game Router, and Realtime Gateway remain
outside this implementation. The [architecture document](docs/design/architecture.md)
distinguishes this implementation from the proposed distributed system.
The [domain event and outbox design](docs/design/domain-events.md) explains event
ownership, publication guarantees, and publisher locking.
