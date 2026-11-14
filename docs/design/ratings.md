# Reliable post-game ratings

Players have independent Elo states for 600 seconds plus 30-second byoyomi and
zero main time plus 30-second byoyomi. Both start at effective Elo 1500 and use
K32. The calculation rounds Black's delta once and gives White its exact
negation. Ratings have no floor or provisional period. Clocks and timeout
adjudication remain outside this capability.

`rating-domain` owns pool and applied-state types and the pure checked Elo
calculation. `game-application::rating` owns rated admission contracts and worker
policy. The existing game writer owns all admission and terminal transactions.
The existing game record read stack owns anonymous rating retrieval, without
account, broker, actor or writer dependencies.

## Admission and completion

All production games start through [matchmaking](matchmaking.md). The client
chooses a pool and owner-scoped ticket UUID. The server selects the opponent and
sides. Pairing commits game creation, complete frozen `RatingState` values, both
reservations, and both playing tickets in one database transaction. Initial effective Elo is 1500. Rating state rows lock in UserId
order, shared with worker application.

One waiting or playing ticket is allowed per user across both pools. Refusals,
preceding cancellations and terminal tickets remain durable. After a lost
response, clients read the ticket or retry its UUID with the same pool.

Terminal acceptance writes the result, receipt, outbox events, both finished
tickets and one rating job atomically. It requires both valid playing tickets.
The global participation becomes free, while reservations remain until worker
application. Users can join the other pool immediately, but same-pool admission
returns a durable `rating_pending` refusal until that reservation is released.

## Claims and application

A short claim transaction stores the worker owner, increasing generation, lease
expiry and attempt count. Acquisition skips locked jobs. Expired claims count
toward the retry limit. Exhausted jobs enter quarantine without freeing either
reservation. Transient failures use bounded backoff. Corrupt inputs or state
mismatches enter quarantine through the same owner-and-generation fence.

The pure Elo calculation runs outside a transaction. Application locks the job,
then reads actual database time. Unfinished work requires the current owner,
generation and unexpired lease. It locks both state rows in the same order as
admission and verifies the complete frozen states and both game reservations.
Both updated ratings, the unique application ledger, job completion and
reservation release commit together. The completion statement checks database
wall time again after all row-lock waits. Any lost fence rolls the transaction
back. Reservation deletion includes the user, pool and game ID.

A completed job reconciles against its full ledger before an old claim is
rejected. A consistent retry returns `AlreadyApplied` without touching current
ratings or reservations. This also resolves successful commits whose response
was lost. An old retry cannot delete a newer game's reservation. The explicit
[quarantine retry command](../../apps/rating-worker-server/README.md#retry-a-quarantined-game)
resets the budget and advances the generation while preserving starts and slots.

## Public reads and permissions

`GET /users/{user_id}/ratings` exposes the latest committed state in both pools.
No applied state means a null rating and zero completed games, including for a
valid unknown user ID. It does not assert account existence or that pending
updates have finished. Storage failures return 503.

The fresh version 7 schema grants `shogi_reader` SELECT only on `schema_version`,
`games`, `journal` and `rating_states`, when that provisioned role exists.
`rated_games`, `rating_slots`, `rating_jobs`, `rating_applications` and `matchmaking_tickets` remain
private. Provisioning has no blanket future-table grants. Startup works without
the reader role, while deployments provision it before game schema initialization.
Previous schema versions fail startup unchanged. No migration or automatic
data deletion runs.
