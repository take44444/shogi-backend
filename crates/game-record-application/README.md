# game-record-application

`GameRecords` searches completed public records and reads one public history
through `RecordRepository`. The crate depends on shared user IDs and shogi value
types. It has no database, HTTP, game actor, account lookup, or write port.

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
Pages do not promise a frozen snapshot across HTTP requests.

`Metadata` has no history. `Detail` contains metadata and public operations only.
Receipts, retry IDs, expected sequence, and outbox events have no public type here.
Detail is limited to 4096 operations and must come from one consistent repository
snapshot. `Error::HistoryTooLong` reports the limit without truncating success.
The production adapter is `game-persistence::SqliteGameRecords`.
