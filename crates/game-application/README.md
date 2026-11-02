# game-application

`GameService::new(Arc<dyn Store>)` uses the application-owned async `Store` port.
The application and domain contain no SQL or ORM types. Production composition
injects `persistence::SeaOrmStore`. There is no default in-memory store.

`create_user(UserId)` returns `User { id }`; a duplicate ID returns `AlreadyExists`.
`get_user(&UserId)` returns `UserNotFound` for an unknown user.
`create(GameId, Participants { black, white })` binds two distinct existing users
permanently to the game's sides. Equal users return `InvalidParticipants` and
missing users return `UserNotFound`. A duplicate game returns `AlreadyExists`.

`snapshot` returns the participants, sequence, owned position, legal moves,
turn, check, and result. An uncached game loads from durable storage.
Unknown IDs return `NotFound` without retaining actors. A closed actor reloads
the durable game. Each game has one actor per service registry and a mailbox
of 64 commands. Registry locks never span database or channel waits.

`make_move(MakeMove)` and `resign(Resign)` take a `user_id`, request ID, and
required typed `expected_seq: u64`. The service derives the side from the binding.
A nonparticipant returns `NonParticipant` before receipt lookup.
Either participant may resign off turn. Existing domain rules remain unchanged.

Sequence starts at zero and advances once per accepted command, including
resignation. The portable durable limit is `i64::MAX`; exhaustion precedes
candidate mutation. Only accepted commands reserve request IDs. Exact retries
match command kind, user ID, expected sequence, and typed move. They return the
original `Acceptance`, even after later commands or game completion. Changed
contents return `RequestConflict`. Rejected request IDs remain reusable.
Replay precedes stale-sequence and finished-game checks.

Before each snapshot or mutation, the actor checks the durable version and
reloads if it differs or its state was invalidated. A failed storage read returns
an error even when a cached snapshot exists. Restoration replays a coherent,
ordered journal from `Game::new()` and validates contiguous sequence numbers,
request uniqueness, participant binding, legality, every receipt, and final
metadata. Invalid history returns `Corrupt`.

A command stages a cloned `Game` and one accepted receipt. `Store::append`
atomically compares the version, advances metadata, and writes the receipt.
The actor publishes the candidate only after a confirmed commit. A failed append
reloads durable history. A matching receipt returns its durable acceptance;
changed contents return `RequestConflict`. A competing version returns
`SequenceMismatch`. An uncertain result with no receipt returns `Unavailable`,
invalidates local state, and does not claim rollback. A failed reconciliation
also leaves state invalid. A later successful refresh permits recovery.

Cancellation after enqueue does not cancel the actor's transaction. A lost
response or `Unavailable` may mean the command committed. Retry identical
contents with the same request ID. Do not assign a new ID to an uncertain retry.
Dropping the last service closes its senders. Actors drain queued commands and
exit. Finished games remain queryable and replayable across process restarts.

`GameId`, `RequestId`, and `UserId` require 1 through 128 bytes without whitespace.
User IDs are internal caller claims, not authentication credentials.
