# rpc

The `shogi.v1.GameService` gRPC adapter exposes `CreateGame`, `GetSnapshot`,
`MakeMove`, and `Resign`. Generated clients and messages are public in `rpc::proto`.
`GameRpc::new(GameService)` adapts the application service without owning rules.
The source schema is [`proto/game.proto`](../../proto/game.proto).

Snapshots contain occupied squares with absolute files and ranks from 1 through 9,
all seven hand-piece counts for both players including zero counts, and legal
moves in USI notation. Mutations return only `Acceptance`; query `GetSnapshot`
for the current board. `expected_seq` is required and zero is valid. Unspecified
and unknown player enums are rejected. Player identity is supplied by the caller.

| Condition | gRPC code |
| --- | --- |
| Invalid ID, USI syntax, missing sequence, unknown or unspecified player | `InvalidArgument` |
| Missing game | `NotFound` |
| Duplicate game or conflicting accepted request ID | `AlreadyExists` |
| Sequence mismatch | `Aborted` |
| Wrong turn, illegal move, finished game | `FailedPrecondition` |
| Rules failure or sequence exhaustion | `Internal` |
| Lost actor or mailbox | `Unavailable` |

`Aborted` includes the current sequence as decimal `actual-seq` response metadata.
Retry semantics and lifecycle are documented in
[`game-application`](../game-application/README.md). The build uses vendored protoc
through `tonic_prost_build::Config::protoc_executable` and requires no global
protoc installation or environment mutation.
