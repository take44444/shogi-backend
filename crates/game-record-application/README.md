# game-record-application

`GameRecords` searches completed public records and reads one public history
through `RecordRepository`. The crate depends on shared user IDs and shogi value
types. It has no database, HTTP, game actor, account lookup, or write port.

## Search and pagination

`Search::new` validates a required `[from,to)` interval of UTC integer microseconds,
a maximum 366-day span, page limits from 1 to 100, nonnegative signed-64-bit move
bounds, and an ordered inclusive ply range. `Filters` combines participant,
standard rule, winner color, finish reason, period, and final move ply.
A participant matches either color. Ply counts moves and excludes resignation.

Search ordering is descending `(finished_at, binary game ID)`. `Search::cursor`
encodes a versioned seek key and a digest of the normalized filters. A page limit
may change between pages. Other filter changes reject the cursor. Cursors are
untrusted client input and have no MAC or signature. The digest binds ordinary
continuations to their query but is not authentication or tamper protection.
Repositories reapply publication rules and every filter before each page limit.
Search results are not fixed across requests. Each page reads current data.

## Metadata and public history

`OperationSequence` counts accepted commands, including resignation. `MovePly`
counts moves only. They are distinct types and accept nonnegative values up to
`i64::MAX`. `PreparedFinishTime` accepts nonnegative UTC microsecond values; the
type does not establish which clock produced a timestamp.

`Metadata::new` requires distinct players and a nonzero final operation count.
A checkmate has equal move and operation counts. A resignation has one more
operation than moves. The constructor checks these relationships without
replaying the game.

`Metadata` has no history. `Detail` contains metadata and public operations only.
Receipts, retry IDs, expected sequence, and outbox events have no public type here.
Repository implementations must read detail from one consistent database
snapshot and reject histories over `MAX_HISTORY` rather than truncate them.
`MAX_HISTORY` is 4096 operations; it does not limit game commands or metadata
search. `GameRecords` delegates these reads to `RecordRepository`.
The production adapter is
[`game-record-persistence::PostgresGameRecords`](../game-record-persistence/src/lib.rs).

## Verification

Run from the repository root.

```sh
cargo test --locked -p game-record-application
python3 scripts/check-contract-interfaces.py
```

Unit tests cover numeric bounds, metadata consistency, and search and cursor
validation. The interface check rejects interchanging move and operation counts
or modifying private metadata fields. Database history limits are tested in
`game-persistence`.
