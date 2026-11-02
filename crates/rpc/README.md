# RPC reference

The generated `shogi.v1.GameService` API uses plaintext gRPC. The schema is
[`proto/game.proto`](../../proto/game.proto). Vendored protoc builds Rust clients
and servers. `GameRpc::new(GameService)` exposes the application service.

| Method | Required request fields | Response |
|---|---|---|
| `CreateUser` | `user_id` | `User { user_id }` |
| `GetUser` | `user_id` | `User { user_id }` |
| `CreateGame` | `game_id`, `black_user_id`, `white_user_id` | `Snapshot` |
| `GetSnapshot` | `game_id` | `Snapshot` |
| `MakeMove` | `game_id`, `request_id`, `expected_seq`, `user_id`, `usi` | `Acceptance` |
| `Resign` | `game_id`, `request_id`, `expected_seq`, `user_id` | `Acceptance` |

IDs contain 1 through 128 bytes without whitespace. `expected_seq` is protobuf
optional solely to distinguish omission from valid zero. Both mutation methods
reject an omitted sequence. `MakeMove` accepts strict USI move notation.
User IDs are supplied claims; there is no authentication in this API.

Mutation input field 4 and the name `player` are reserved. `MakeMove.user_id` is
field 6 and `Resign.user_id` is field 5. Clients choose a bound user, and the server
derives the side. The `Player` enum remains in outputs. Snapshot fields 9 and 10
are `black_user_id` and `white_user_id`. Existing output field numbers are retained.

`Snapshot` contains an occupied-square board, all fourteen hand counts, turn,
check, optional result, legal USI moves, participant IDs, and sequence.
Finished snapshots retain the position and turn with no legal moves.
`Acceptance` contains the accepted sequence, check, and optional result.
Exact retries return the original acceptance, including after restart or finish.

| Application outcome | gRPC code |
|---|---|
| Malformed ID, move, missing sequence, or equal participants | `InvalidArgument` |
| Unknown user or game | `NotFound` |
| User is not a participant | `PermissionDenied` |
| Duplicate user or game, or conflicting accepted request ID | `AlreadyExists` |
| Stale sequence | `Aborted`, with `actual-seq` metadata |
| Wrong turn, illegal move, or finished game | `FailedPrecondition` |
| Storage failure or uncertain outcome | `Unavailable` |
| Corrupt durable history or incompatible schema | `DataLoss` |
| Rules implementation failure or sequence exhaustion | `Internal` |

After `Unavailable` or a lost response, retry the same request ID with identical
contents. Cancellation after enqueue does not cancel a durable command.

The `play` example verifies a complete game. `persistence_proof` has separate
`seed`, `resume`, and `finished` invocations for restart testing with protobuf
artifacts. The [root README](../../README.md) contains runnable commands.
