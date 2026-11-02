# persistence

`SeaOrmStore::connect(&str).await` connects and migrates before returning a Store.
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

Each SQLite connection enables foreign keys, WAL, full synchronization, and a
five-second busy timeout. In-memory databases use one pooled connection.
File databases use up to five. SQLite settings live only in this adapter.
Database errors return `Unavailable`; an incompatible schema version returns
`Corrupt`.

Startup acquires an immediate transaction before inspecting the schema. Only a
new database receives a `schema_version` ledger. Ledger creation and version 1
are committed together. An existing empty ledger, a missing ledger beside game
tables, missing application tables, and unknown versions fail closed without
repairing metadata. Concurrent startup serializes migration.

Game creation and append use immediate SQLite transactions. Append updates the
game only at the expected version and inserts the receipt in the same transaction.
It verifies participant membership before committing. A failed statement rolls
back explicitly. SeaORM retains an open failed transaction until its drop queues
SQLx rollback; such a connection cannot serve the next request with pending writes.
The application reconciles every append failure through a fresh durable load.

`load_game` reads metadata and the ordered journal with one left join. A load
cannot combine metadata and history from different database snapshots. The
application validates replay; the adapter never restores a game from an untrusted
position string. Direct manual database edits are outside the mutation API.

Contract tests use both in-memory and temporary file SQLite databases. They cover
constraints, concurrent bootstrap and writers, coherent reads, real statement
and deferred-commit failures, corrupt history, uncertain commits, failed
reconciliation, exact replay, and process-independent service restoration.
