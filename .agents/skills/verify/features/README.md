# Shogi API verification map

Read the feature recipe before driving. This map is the maintained source of user paths, including paths the automated helper cannot currently reach.

## Baseline

Run `./scripts/verify-app.py FEATURE` from the repository root. It builds current sources, creates a fresh PostgreSQL database in a dedicated Compose project, starts game-server and player-server, runs doctor, drives the journey and cleans up. Every invocation gets new users and ports. Evidence is printed under `target/verification/shogi-verify-<id>/`. No credentials or existing database are used. Browser realtime has its own preconditions.

## Proof reporting

Pair commands/actions with read-back observations. Include feature ID, entry point, evidence directory and cleanup result. Preserve evidence after teardown. Report missing preconditions as a failed attempted journey with the reason; label untouched branches unverified. One entry point does not establish another. Do not treat integration tests or direct gRPC as browser proof.

## Features

- [Play a game](gameplay.md): matchmaking, promotion, capture, drop, resignation and exact retries through internal gRPC.
- [Resume after restart](persistence.md): in-progress and completed state, replaying original receipts.
- [Read public records](records.md): filtered HTTP search, detail, missing records and invalid queries.
- [Read committed ratings](ratings.md): initial state, worker application, separate rating pools.
- [Accounts and browser realtime](browser-realtime.md): Google login, profile/logout, ticket lifecycle and authenticated WebSocket commands; requires real Google accounts and browser access.

Outbox failure/recovery also has an existing live harness, `scripts/prove-outbox.sh`, documented in the root README. It requires Valkey and uses its own Compose project; it is not run by `verify-app.py`. Natural clock timeout, full pagination, cancellation/expiry branches, spectating/access denial, and quarantined rating retry need additional journeys when those behaviors change. Never infer coverage for them from the four automated commands.
