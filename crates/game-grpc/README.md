# game-grpc

The generated `shogi.v1.GameService` API uses plaintext gRPC. The schema is
[`proto/game.proto`](../../proto/game.proto). Vendored protoc builds Rust clients
and servers. `GameRpc::new(GameService, Arc<dyn MatchmakingRepository>)` dispatches game operations.
User management belongs to the separate HTTP service and has no gRPC methods.

`GetSnapshotForViewer(game_id, viewer_user_id)` and `CheckViewAccess(game_id, viewer_user_id)` serve trusted gateway callers. The former reads visibility, participants, and history in one coherent database view before reconstructing a snapshot. The latter checks current visibility and participants without loading history. Public games allow any authenticated viewer; private games allow only participants. Missing and inaccessible games both return `NotFound`. The caller must authenticate the viewer before supplying `viewer_user_id`; the gRPC listener itself remains an unauthenticated internal API.

## Requests and responses

| Method | Required request fields | Response |
|---|---|---|
| `JoinMatchmaking` | authenticated `user_id`, UUID `ticket_id`, `pool` | Ticket and first-admission flag |
| `GetMatchmaking` | authenticated `user_id` | Active ticket or idle |
| `GetMatchmakingTicket` | authenticated `user_id`, UUID `ticket_id` | Stored ticket |
| `HeartbeatMatchmaking` | authenticated `user_id`, UUID `ticket_id`, positive `sequence` | Current ticket |
| `CancelMatchmaking` | authenticated `user_id`, UUID `ticket_id` | Current ticket, including preceding cancellation |
| `GetSnapshot` | `game_id` | `Snapshot` |
| `GetSnapshotForViewer` | `game_id`, authenticated `viewer_user_id` claim | `Snapshot` or `NotFound` when absent or inaccessible |
| `CheckViewAccess` | `game_id`, authenticated `viewer_user_id` claim | Empty success or `NotFound` when absent or inaccessible |
| `MakeMove` | `game_id`, `request_id`, `expected_seq`, `user_id`, `usi` | `Acceptance` |
| `Resign` | `game_id`, `request_id`, `expected_seq`, `user_id` | `Acceptance` |

IDs contain 1 through 128 bytes without whitespace. `expected_seq` is protobuf
optional solely to distinguish omission from valid zero. Both mutation methods
reject an omitted sequence. `MakeMove` accepts strict USI move notation.
User IDs are supplied claims. This API is unauthenticated and intended for trusted
internal callers. Matchmaking determines the opponent, game ID and sides.
The HTTP user service does not authenticate these gRPC calls.

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
| Malformed ID, move, missing sequence, or invalid heartbeat sequence | `InvalidArgument` |
| Unknown game, or a viewer API called by an unauthorized viewer | `NotFound` |
| User is not a participant | `PermissionDenied` |
| Ticket pool conflict, or conflicting accepted request ID | `AlreadyExists` |
| Stale sequence | `Aborted`, with `actual-seq` metadata |
| Wrong turn, illegal move, or finished game | `FailedPrecondition` |
| Storage failure or uncertain outcome | `Unavailable` |
| Corrupt durable history or incompatible schema | `DataLoss` |
| Internal invariant violation, rules implementation failure, or sequence exhaustion | `Internal` |

After `Unavailable` or a lost response, retry the same request ID with identical
contents. Cancellation after enqueue does not cancel a durable command.

Matchmaking refusals are successful RPC responses with `refused` state and the
saved reason. The HTTP gateway maps a join refusal to 409. Unknown owner-scoped
tickets return `NotFound`. Reusing a UUID with another pool returns `AlreadyExists`.
After an uncertain join, retry the same UUID and pool or read its ticket. No
production RPC or application method creates games from arbitrary participants.

## Examples and verification

The `play` example exercises moves and resignation. `persistence_proof` has separate
`seed`, `resume`, and `finished` invocations for restart testing with protobuf
artifacts. The [root README](../../README.md) contains runnable commands.

Run adapter tests from the repository root.

```sh
cargo test --locked -p game-grpc
```
