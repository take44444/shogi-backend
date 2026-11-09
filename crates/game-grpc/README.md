# game-grpc

The generated `shogi.v1.GameService` API uses plaintext gRPC. The schema is
[`proto/game.proto`](../../proto/game.proto). Vendored protoc builds Rust clients
and servers. `GameRpc::new(GameService)` dispatches game operations.
User management belongs to the separate HTTP service and has no gRPC methods.

## Requests and responses

| Method | Required request fields | Response |
|---|---|---|
| `CreateGame` | `game_id`, `black_user_id`, `white_user_id` | `Snapshot` |
| `GetSnapshot` | `game_id` | `Snapshot` |
| `MakeMove` | `game_id`, `request_id`, `expected_seq`, `user_id`, `usi` | `Acceptance` |
| `Resign` | `game_id`, `request_id`, `expected_seq`, `user_id` | `Acceptance` |

IDs contain 1 through 128 bytes without whitespace. `expected_seq` is protobuf
optional solely to distinguish omission from valid zero. Both mutation methods
reject an omitted sequence. `MakeMove` accepts strict USI move notation.
User IDs are supplied claims. This API is unauthenticated and intended for trusted
internal callers. Creating a game requires two valid, distinct IDs and performs no
account lookup. The HTTP user service does not authenticate these gRPC calls.

Clients supply a bound user ID, and the server derives the side. The `Player`
enum remains in outputs. Reserved fields and wire numbers are defined in
[`game.proto`](../../proto/game.proto).

`Snapshot` contains an occupied-square board, all fourteen hand counts, turn,
check, optional result, legal USI moves, participant IDs, and sequence.
Finished snapshots retain the position and turn with no legal moves.
`Acceptance` contains the accepted sequence, check, and optional result.
Exact retries return the original acceptance, including after restart or finish.

## Errors and retries

| Application outcome | gRPC code |
|---|---|
| Malformed ID, move, missing sequence, or equal participants | `InvalidArgument` |
| Unknown game | `NotFound` |
| User is not a participant | `PermissionDenied` |
| Duplicate game, or conflicting accepted request ID | `AlreadyExists` |
| Stale sequence | `Aborted`, with `actual-seq` metadata |
| Wrong turn, illegal move, or finished game | `FailedPrecondition` |
| Storage failure or uncertain outcome | `Unavailable` |
| Corrupt durable history or incompatible schema | `DataLoss` |
| Internal invariant violation, rules implementation failure, or sequence exhaustion | `Internal` |

After `Unavailable` or a lost response, retry the same request ID with identical
contents. Cancellation after enqueue does not cancel a durable command.

## Examples and verification

The `play` example exercises moves and resignation. `persistence_proof` has separate
`seed`, `resume`, and `finished` invocations for restart testing with protobuf
artifacts. The [root README](../../README.md) contains runnable commands.

Run adapter tests from the repository root.

```sh
cargo test --locked -p game-grpc
```
