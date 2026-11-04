# game-persistence

`SeaOrmStore::connect(&str).await` connects to PostgreSQL and initializes or validates the game schema before returning the shared adapter. The database and role must already exist. `SeaOrmStore` implements the backend-neutral `GameRepository` port. Its database contains game state and the durable event outbox. It does not query or connect to the user service. SeaORM 2.0.4 is pinned with default features disabled and the PostgreSQL driver enabled.

Ordinary data operations and table/index definitions use SeaQuery builders. SeaORM executes its generated statements directly. The record reader and Outbox retain their dedicated SQLx connections and use `sea-query-sqlx` to pass generated SQL and bound values separately. Handwritten SQL is limited to PostgreSQL locks, connection settings, catalog inspection, and the `C` collation expression. Test fixtures retain SQL for deliberate corruption and compatibility with the previous schema.

## Game storage

`games` stores immutable participant IDs, the current version, and the final result. `journal` stores each accepted request's user, expected sequence, typed action encoded as canonical USI or resignation, and receipt. Foreign keys bind each journal entry to its game and each outbox row to its journal entry. A distinct-participant check rejects equal bindings. Composite uniqueness protects game sequence and game request ID. Result-pair checks reject either missing half of a terminal result. Identifiers use an order-preserving escape for NUL and the escape byte because PostgreSQL TEXT excludes NUL. The adapter decodes them before returning application values. All durable sequence conversions check the signed 64-bit range.

Schema startup serializes initialization with a PostgreSQL advisory lock inside the schema transaction. An empty database receives the version 5 ledger, tables, constraints, and indexes atomically. It does not create account tables. Existing databases must contain the current ledger and required application tables. An empty or malformed ledger, a missing ledger, missing application tables, and any other version fail closed without changing existing data. Startup does not upgrade, rebuild, or delete existing data. Database errors return `Unavailable`; an incompatible schema returns `Corrupt`.

Game creation and `commit_change` use PostgreSQL transactions. `commit_change(&StagedGameChange)` updates the game only at the expected version and inserts the supplied receipt and already generated events in the same transaction. The application constructs the change and freezes event content before commit. The adapter does not generate events. It verifies participant membership before committing. A failed statement rolls back the transaction. The application reconciles every commit failure through a fresh durable load.

`load_game` reads metadata and the ordered journal with one left join. A load cannot combine metadata and history from different database snapshots. The application validates replay. The adapter never restores a game from an untrusted position string. Direct manual database edits are outside the mutation API.

## Outbox ownership

`outbox_events` freezes each event's typed payload and encoding version at commit time. Pending events are not regenerated from journal records during delivery. Uniqueness on `(game_id, seq, event_kind)` protects event identity, and a foreign key binds the event to its journal entry. The `published` flag records acceptance of a transient notification by the broker. It does not record subscriber delivery or durable job completion. Published rows remain stored. A pending index orders dispatch by insertion ID. That ID is a storage cursor, separate from the public event identity.

`PostgresOutbox::connect` acquires a session advisory lock on a dedicated PostgreSQL connection. A competing publisher for the same game database fails startup. All queue reads and acknowledgements use the connection that owns the lock. Losing that connection makes later queue operations fail. The worker exits when it observes the failure. The lock permits ordinary game transactions, and dispatch holds no transaction open while waiting for Valkey. Closing the connection releases ownership.

The application stops its Tokio runtime before releasing the queue connection. This stops local background communication but cannot cancel a `PUBLISH` already sent to Valkey. After a PostgreSQL session loss, an in-flight command may still execute remotely while its row remains pending. A replacement publisher may retry it, so the advisory lock does not provide a strict fence against delayed duplicates or arrival order changes across hosts.

The publisher reads one pending row and marks that exact row after a successful transport handoff. Marking is idempotent. A failed or uncertain handoff or mark leaves recovery to the next read. This single publisher preserves the order of new event attempts, but uncertain sends can produce late duplicates. A blocked head event blocks later events. The adapter does not implement parallel delivery workers or publisher leases.

## Public completed records

`PostgresGameRecords::connect(&url)` implements the separate `game-record-application` read port. It uses a dedicated PostgreSQL pool and never initializes a schema or exposes a game write repository. Startup requires schema version 5 and the expected game and journal columns. The pool has four connections, a two-second acquisition timeout, and a two-second statement timeout. Connections default to read-only transactions.

Deployments must also restrict PostgreSQL role permissions. [The bootstrap SQL](../../scripts/init-postgres.sql) creates `shogi_reader` with SELECT grants on tables owned by the game writer. It grants neither table writes nor object ownership. The role remains unable to mutate game tables if a session disables the read-only transaction default. Compose's `records` profile passes this role's credentials through `PLAYER_GAME_DATABASE_URL`.

Version 5 requires non-null `rule`, `visibility`, and `final_move_ply`. Only an ongoing game may have a null `finished_at`; a completed game must have a finish timestamp and a complete result. New games explicitly use `standard`, `public`, and a zero move count. Accepted moves increment the count. Resignation advances sequence without advancing moves. A terminal commit stores UTC microseconds from the writer's wall clock inside the transaction that updates version, journal, and outbox. The timestamp marks commit preparation. An exact retry returns its original acceptance and does not update metadata.

Search selects game metadata only and applies public completion, period, and all filters before the limit. Partial indexes support finished-time ordering and participant searches. Detail uses one consistent database snapshot for metadata and a bounded ordered journal. It validates IDs, action syntax, participant colors, alternating move turns, contiguous sequences, move counts, and terminal result consistency. It does not replay move legality. Neither query returns request IDs, expected sequences, acceptance receipts, or outbox data.

## Verification

Contract tests use isolated PostgreSQL databases through `pg-test-support`. They cover constraints, concurrent initialization and writers, coherent reads, transaction failures, corrupt history, uncertain commits, failed reconciliation, exact replay, reader permissions, publisher ownership, and service restoration. Run `./scripts/test-postgres.sh cargo test --locked -p game-persistence` from the workspace root.
