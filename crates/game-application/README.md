# game-application

`GameService` owns independent, in-memory shogi games through a shared registry.
Clones share that registry. `create` inserts a new game or returns `AlreadyExists`.
`snapshot` returns an owned position, legal moves, turn, check, result, and sequence.

`make_move` accepts `MakeMove`; `resign` accepts `Resign`. Both return a compact
`Acceptance`. Sequence starts at zero and advances once per accepted command,
including resignation. A rejected command changes neither sequence nor game state.

Each game has one private actor and a mailbox of 64 commands. Sending waits for
capacity. The registry lock is never held across a channel wait. Games do not
block each other's mailboxes. Dropping the last service closes the registry's
senders; actors drain enqueued commands and exit. Finished games remain available
for snapshots and retries while the service lives. An actor failure returns
`Unavailable` and never recreates the game.

Request IDs are unique within a game across moves and resignation. Only accepted
commands reserve IDs. Repeating the same ID and exact typed contents returns the
original `Acceptance`, even after later moves or game completion. Changing the
kind, player, expected sequence, or move returns `RequestConflict`. Replays do not
return the current snapshot. Query `snapshot` to synchronize.

Cancellation after enqueue does not cancel a command. A lost response or
`Unavailable` can leave the outcome unknown. Retry identical contents with the
same request ID against the same live service. State and replay history are kept
only in memory, without eviction, for the service lifetime. Restart loses both.

`GameId::new` and `RequestId::new` require 1 through 128 bytes without whitespace.
Domain value types come from `shogi-domain`. There are no clocks, storage,
transport dependencies, authentication, or additional adjudication rules here.
