# game-persistence

`SeaOrmStore::connect(&str).await` connects and initializes or validates the schema before returning the shared adapter.
`SeaOrmStore` implements `GameRepository`. Its database contains only game state
and the durable event outbox. It does not query or connect to the user service.
SeaORM 2.0.4 is pinned with default features disabled. SQLite is the only compiled
and tested database driver. No PostgreSQL service or driver is required.
The port is backend neutral; another adapter or driver needs its own validation.

The adapter uses SeaORM connections and SeaQuery statements.
`games` stores immutable participant IDs, the current version,
and the final result. `journal` stores each accepted request's user, expected
sequence, typed action encoded as canonical USI or resignation, and receipt.
Foreign keys bind each journal entry to its game and each outbox row to its journal entry. A distinct-participant check rejects equal
bindings. Composite uniqueness protects game sequence and game request ID.
Explicit result-pair checks reject either missing half of a terminal result.
All durable sequence conversions check the signed 64-bit range.

Each SQLite connection enables foreign keys, full synchronization, and a
five-second busy timeout. File databases use WAL; in-memory databases use memory
journaling. In-memory databases use one pooled connection.
File databases use up to five. SQLite settings live only in this adapter.
Database errors return `Unavailable`; an incompatible schema version returns
`Corrupt`.

Startup acquires an immediate transaction before inspecting the schema. An empty
database receives the version 5 ledger, tables, constraints, and indexes atomically.
It does not create account tables. Existing databases must contain the current
ledger and required application tables. An empty or malformed ledger, a missing
ledger, missing application tables, and any other version fail closed without
changing existing data. Startup does not upgrade, rebuild, or delete existing data.
Concurrent startup serializes initialization.

Game creation and `commit_change` use immediate SQLite transactions.
`commit_change(&StagedGameChange)` updates the game only at the expected version and
inserts the supplied receipt and already generated events in the same transaction.
The application constructs the change and freezes event content before commit;
the adapter does not generate events.
It verifies participant membership before committing. A failed statement rolls
back explicitly. SeaORM retains an open failed transaction until its drop queues
SQLx rollback; such a connection cannot serve the next request with pending writes.
The application reconciles every commit failure through a fresh durable load.

`load_game` reads metadata and the ordered journal with one left join. A load
cannot combine metadata and history from different database snapshots. The
application validates replay; the adapter never restores a game from an untrusted
position string. Direct manual database edits are outside the mutation API.

`outbox_events` freezes each event's typed payload and encoding version at commit
time. Pending events are not regenerated from journal records during delivery.
Uniqueness on `(game_id, seq, event_kind)` protects event identity, and a foreign
key binds the event to its journal entry. Published rows remain stored. A pending
index orders dispatch by insertion ID. That ID is a storage cursor, separate
from the public event identity.

`SqliteOutbox::connect` is the production entry point to pending events. It
requires a file-backed database on a Linux local filesystem and holds an
exclusive OS file lock on the actual database inode. Competing publishers
cannot open the queue concurrently. Use the same database path for all services.
Hard links to a live WAL database are unsupported because they can use different
WAL files for the same database inode.
The lock permits ordinary SQLite transactions, and dispatch holds no transaction
open while waiting for NATS. Process exit releases the lock. Replacing or
unlinking a live database is unsupported.

The publisher reads one pending row and marks that exact row after a successful
transport handoff. Marking is idempotent. A failed or uncertain handoff or mark
leaves recovery to the next read. This single publisher preserves the order of
new event attempts, but uncertain sends can produce late duplicates. A blocked
head event blocks later events. This adapter does not implement distributed
publisher ownership or leases.

Contract tests use both in-memory and temporary file SQLite databases. They cover
constraints, concurrent bootstrap and writers, coherent reads, real statement
and deferred-commit failures, corrupt history, uncertain commits, failed
reconciliation, exact replay, and process-independent service restoration.

## Public completed records

`SqliteGameRecords::open(path)` implements the separate `game-record-application`
read port. It canonicalizes an existing file, opens SQLite with read-only flags,
and enables `query_only` on every pooled connection. It never creates a file,
initializes a schema, changes journal mode, or exposes a game write repository.
Startup requires schema version 5 and the expected game and journal columns.
The reader uses SQLx from SeaORM's existing driver dependency for SQLite progress
handlers. The pool has four connections, a two-second acquisition timeout,
a two-second busy timeout, and a two-second SQLite instruction deadline per request.
The progress handler interrupts database work, including work left by a cancelled request.
The next request resets the handler before reusing a connection.

Version 5 requires non-null `rule`, `visibility`, and `final_move_ply`.
Only an ongoing game may have a null `finished_at`; a completed game must have
a finish timestamp and a complete result.
New games explicitly use `standard`, `public`, and a zero move count. Accepted
moves increment the count. Resignation advances sequence without advancing moves.
A terminal commit stores UTC microseconds from the writer's wall clock inside
the transaction that updates version, journal, and outbox. The timestamp marks
commit preparation, rather than an exact physical commit instant. An exact retry
returns its original acceptance and does not update metadata.


Search selects game metadata only and applies public completion, period, and all
filters before the limit. Partial indexes support finished-time ordering and
participant searches. Detail uses one read transaction for metadata and a bounded
ordered journal. It validates IDs, action syntax, participant colors, alternating
move turns, contiguous sequences, move counts, and terminal result consistency.
It does not replay move legality. Neither query returns request IDs, expected
sequences, acceptance receipts, or outbox data.

The deployment must also grant only read access to the database directory and
existing WAL and SHM files. Compose's `records` profile mounts the shared volume
read-only. Start the game writer first and retain its live WAL files. SQLite
read-only WAL access requires readable, initialized WAL and SHM files. Do not
replace or unlink a database while either process uses it. Filesystem permissions
and container volume access remain deployment responsibilities.
