# Shogi backend

Run a durable shogi game over gRPC with SQLite and a generated Rust client.
Rust and Cargo are required for the local commands. Cargo supplies vendored protoc.

Start the server from the repository root.

```sh
cargo run -p game-server
```

In another terminal, run the client.

```sh
cargo run -p rpc --example play
```

The client creates two users and a game, plays `7g7f 3c3d 8h2b+ 3a2b B*4e`,
and resigns White. It verifies capture, promotion, recapture, a bishop drop,
snapshots, and exact retries. The final line reports `black wins at sequence 6`.
Each invocation needs a new game ID. Omitting the ID generates one.

The server defaults to `127.0.0.1:50051` and
`DATABASE_URL=sqlite://data/shogi.db?mode=rwc`. Startup creates the database's
parent directory and migrates the schema before listening. Users, participant
bindings, positions, results, and accepted request receipts survive restart.
A positional address overrides `GAME_SERVER_ADDR`.

```sh
DATABASE_URL='sqlite://data/demo.db?mode=rwc' cargo run -p game-server -- 127.0.0.1:50052
cargo run -p rpc --example play -- http://127.0.0.1:50052 my-game
```

## Prove persistence across container recreation

Build and start the server. Compose binds its port to the host loopback address
and stores SQLite in the `shogi-data` named volume. Set `GAME_SERVER_PORT` to
change the host port. These commands assume the default port.

```sh
docker compose up --build -d
cargo run -p rpc --example persistence_proof -- seed http://127.0.0.1:50051 restart-proof /tmp/shogi-proof
docker compose up -d --force-recreate game-server
cargo run -p rpc --example persistence_proof -- resume http://127.0.0.1:50051 restart-proof /tmp/shogi-proof
docker compose restart game-server
cargo run -p rpc --example persistence_proof -- finished http://127.0.0.1:50051 restart-proof /tmp/shogi-proof
```

Wait for `game-server listening on 0.0.0.0:50051` in
`docker compose logs game-server` before each client invocation. Use a fresh game
ID and proof directory for another run. The client stores protobuf snapshots and
receipts in the proof directory, compares them after restart, continues play,
and verifies both original receipts after a second restart of the finished game.

`docker compose down` stops the server while retaining its database volume.
The same seed, resume, and finished commands also work with a local server.
Stop and restart the local server between phases with the same `DATABASE_URL`.

## Verify the workspace

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The [application contract](crates/game-application/README.md) covers participant
binding, sequencing, replay, cancellation, and recovery. The
[storage reference](crates/persistence/README.md) covers SQLite transactions and
schema startup. The [RPC reference](crates/rpc/README.md) covers wire fields and
status codes. The [domain crate](crates/shogi-domain/README.md) owns shogi rules.

User IDs are caller-supplied identity claims. This service has no authentication.
It adds no matchmaking, shards, messaging, clocks, or new adjudication rules.
The older [architecture proposal](architecture.md) describes future systems.
It does not describe dependencies required to run this implementation.
