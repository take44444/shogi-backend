# game-persistence

`SeaOrmStore::connect(&str).await` connects to PostgreSQL and initializes or
validates the game schema. `SeaOrmStore` implements `GameRepository` for game
state, accepted requests, and the durable event outbox. Account storage belongs
to the separate user service.

Data operations, table definitions, and index definitions use SeaQuery builders.
SeaORM executes generated statements. The outbox uses a dedicated SQLx connection
and `sea-query-sqlx` to keep values separate from SQL text. PostgreSQL-specific
operations use handwritten SQL. Test fixtures also use SQL to inject corruption
and check compatibility with the previous schema.

## Schema startup

The database and role must already exist. The role needs database `CREATE`
permission and permission to create objects in the target schema. Existing-schema
validation also requires database `CREATE`: it builds a reference schema inside
a savepoint and rolls it back after comparison.

Startup takes a PostgreSQL transaction advisory lock. An empty schema receives
the version 5 ledger, tables, constraints, and indexes atomically. Existing
schemas must have the current ledger and required definitions, including column
types, NULL constraints, keys, CHECK constraints, and indexes. Extra columns,
indexes, and unrelated tables are permitted when required definitions remain.

An incompatible schema returns `Corrupt`; database failures return `Unavailable`.
Startup does not upgrade, rebuild, repair, or delete existing data. The reference
schema uses the same creation function as initialization to keep the expected
definitions in one place.

## Game storage

`games` stores immutable participant IDs, the current version, and the final result. `journal` stores each accepted request's user, expected sequence, typed action encoded as canonical USI or resignation, and receipt. Foreign keys bind each journal entry to its game and each outbox row to its journal entry. A distinct-participant check rejects equal bindings. Composite uniqueness protects game sequence and game request ID. Result-pair checks reject either missing half of a terminal result. Identifiers use an order-preserving escape for NUL and the escape byte because PostgreSQL TEXT excludes NUL. The adapter decodes them before returning application values. All durable sequence conversions check the signed 64-bit range.

Game creation and `commit_change` use PostgreSQL transactions. `commit_change(&StagedGameChange)` updates the game only at the expected version and inserts the supplied receipt and already generated events in the same transaction. The application constructs the change and freezes event content before commit. The adapter does not generate events. It verifies participant membership before committing. A failed statement rolls back the transaction. The application reconciles every commit failure through a fresh durable load.

`load_game` reads metadata and the ordered journal with one left join. A load cannot combine metadata and history from different database snapshots. The application validates replay. The adapter never restores a game from an untrusted position string. Direct manual database edits are outside the mutation API.

## Outbox ownership

`outbox_events` freezes each event's typed payload and encoding version at commit time. Pending events are not regenerated from journal records during delivery. Uniqueness on `(game_id, seq, event_kind)` protects event identity, and a foreign key binds the event to its journal entry. The `published` flag records acceptance of a transient notification by the broker. It does not record subscriber delivery or durable job completion. Published rows remain stored. A pending index orders dispatch by insertion ID. That ID is a storage cursor, separate from the public event identity.

`PostgresOutbox::connect` acquires a session advisory lock on a dedicated PostgreSQL connection. A competing publisher for the same game database fails startup. All queue reads and acknowledgements use the connection that owns the lock. Losing that connection makes later queue operations fail. The worker exits when it observes the failure. The lock permits ordinary game transactions, and dispatch holds no transaction open while waiting for Valkey. Closing the connection releases ownership.

The application stops its Tokio runtime before releasing the queue connection. This stops local background communication but cannot cancel a `PUBLISH` already sent to Valkey. After a PostgreSQL session loss, an in-flight command may still execute remotely while its row remains pending. A replacement publisher may retry it, so the advisory lock does not provide a strict fence against delayed duplicates or arrival order changes across hosts.

The publisher reads one pending row and marks that exact row after a successful transport handoff. Marking is idempotent. A failed or uncertain handoff or mark leaves recovery to the next read. This single publisher preserves the order of new event attempts, but uncertain sends can produce late duplicates. A blocked head event blocks later events. The adapter does not implement parallel delivery workers or publisher leases.

## Completed-game metadata

The read-only adapter lives in [game-record-persistence](../game-record-persistence/src/lib.rs). Its production dependency graph excludes this writer crate and the Game Actor. Deployments use the separate reader role from [the bootstrap SQL](../../scripts/init-postgres.sql).

Version 5 requires non-null `rule`, `visibility`, and `final_move_ply`. Only an
ongoing game may have a null `finished_at`; a completed game requires a timestamp
and complete result. New games use `standard`, `public`, and zero moves.
Accepted moves increment the move count. Resignation advances the operation
sequence without incrementing moves.

A terminal commit samples UTC microseconds from the writer's wall clock inside
the transaction that updates the game, journal, and outbox. The timestamp marks
commit preparation, not the database commit instant. An exact retry returns its
original acceptance without changing metadata.

The writer has no 4096-operation limit. That limit belongs to public detail
reads. Durable sequences must fit in a signed 64-bit integer.

## Verification

Contract tests use isolated PostgreSQL databases through `pg-test-support`. They cover constraints, concurrent initialization and writers, coherent reads, transaction failures, corrupt history, uncertain commits, failed reconciliation, exact replay, reader permissions, publisher ownership, and service restoration. Run `./scripts/test-postgres.sh cargo test --locked -p game-persistence` from the workspace root.
