# persistence

`SeaOrmStore::connect(&str).await` connects and migrates before returning the shared adapter.
`SeaOrmStore` implements `UserRepository` and `GameRepository` separately. Both
implementations use the same connection pool; application services receive only
the repository trait they need.
SeaORM 2.0.4 is pinned with default features disabled. SQLite is the only compiled
and tested database driver. No PostgreSQL service or driver is required.
The port is backend neutral; another adapter or driver needs its own validation.

The adapter uses SeaORM connections and SeaQuery statements. The `users` table
stores IDs. `games` stores immutable participant bindings, the current version,
and the final result. `journal` stores each accepted request's user, expected
sequence, typed action encoded as canonical USI or resignation, and receipt.
Foreign keys protect users and games. A distinct-participant check rejects equal
bindings. Composite uniqueness protects game sequence and game request ID.
Explicit result-pair checks reject either missing half of a terminal result.
All durable sequence conversions check the signed 64-bit range.

Each SQLite connection enables foreign keys, full synchronization, and a
five-second busy timeout. File databases use WAL; in-memory databases use memory
journaling. In-memory databases use one pooled connection.
File databases use up to five. SQLite settings live only in this adapter.
Database errors return `Unavailable`; an incompatible schema version returns
`Corrupt`.

Startup acquires an immediate transaction before inspecting the schema. Only a
new database receives a `schema_version` ledger. New databases reach version 2
atomically. Version 1 databases gain an empty `outbox_events` table and advance
to version 2 in the same transaction. Existing users and game history remain
unchanged. Historical commands are not backfilled as live events. An existing
empty ledger, a missing ledger beside owned tables, missing application tables,
an unexpected outbox beside version 1, and unknown versions fail closed without
repairing metadata. Concurrent startup serializes migration.

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
