# Shogi backend

Run an in-memory shogi game over gRPC with a generated Rust client.
Rust is required. Cargo downloads dependencies and provides vendored protoc.

1. Start the server from the repository root.

   ```sh
   cargo run -p game-server
   ```

2. In another terminal, run the generated-client example.

   ```sh
   cargo run -p rpc --example play
   ```

   The example creates a fresh game, plays `7g7f 3c3d 8h2b+ 3a2b B*4e`,
   and resigns White. It verifies capture, promotion, demotion on recapture,
   a bishop drop, snapshots, and exact retries after the game ends.
   The final line reports `black wins at sequence 6`.

3. Press Ctrl-C in the server terminal to stop it.

The server binds to `127.0.0.1:50051`. A positional address overrides
`GAME_SERVER_ADDR`, which overrides that default.

```sh
cargo run -p game-server -- 127.0.0.1:50052
cargo run -p rpc --example play -- http://127.0.0.1:50052 my-game
```

Each example invocation needs a new game ID. Omitting the ID generates one.
All games and accepted-request history disappear when the server exits.

For manual clients such as grpcurl, use [`proto/game.proto`](proto/game.proto).
The server uses plaintext gRPC and the schema is supplied explicitly.

```sh
grpcurl -plaintext -import-path proto -proto game.proto -d '{"gameId":"manual"}' 127.0.0.1:50051 shogi.v1.GameService/CreateGame
grpcurl -plaintext -import-path proto -proto game.proto -d '{"gameId":"manual","requestId":"first","expectedSeq":"0","player":"PLAYER_BLACK","usi":"7g7f"}' 127.0.0.1:50051 shogi.v1.GameService/MakeMove
grpcurl -plaintext -import-path proto -proto game.proto -d '{"gameId":"manual"}' 127.0.0.1:50051 shogi.v1.GameService/GetSnapshot
```

The [application contract](crates/game-application/README.md) describes sequencing,
accepted-only replay, cancellation, and lifecycle. The [RPC reference](crates/rpc/README.md)
describes messages and status codes. The existing [domain crate](crates/shogi-domain/README.md)
owns shogi rules. The implementation adds no database, clocks, authentication,
NATS, shards, or adjudication rules beyond the domain crate.

Run verification from the repository root.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
