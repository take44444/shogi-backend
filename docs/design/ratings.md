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

`CreateGame` creates casual games. `CreateRatedGame` requires an explicit pool.
Admission inserts both state rows if absent, locks them in stable user-ID order,
checks the unique user-and-pool reservations, and freezes starts in the same
transaction as game creation and both reservations. A second game involving
either user in the same pool returns `FailedPrecondition` until the previous
job is applied. Different pools have separate state rows and reservations.

Creation is not idempotent. `GetRatedGame` returns immutable participants, pool
and start states from durable storage for an uncertain create response. Compare
the complete identity with the original request. A conflicting existing ID is
not a successful retry. Casual and absent games have no rated identity.

The existing game acceptance transaction writes the terminal result, receipt,
outbox events and one rating job atomically. Casual completion writes no job.
The worker reads the committed result, independently of broker delivery.

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

The fresh version 6 schema grants `shogi_reader` SELECT only on `schema_version`,
`games`, `journal` and `rating_states`, when that provisioned role exists.
`rated_games`, `rating_slots`, `rating_jobs` and `rating_applications` remain
private. Provisioning has no blanket future-table grants. Startup works without
the reader role, while deployments provision it before game schema initialization.
Previous schema versions fail startup unchanged. No migration or automatic
data deletion runs.
