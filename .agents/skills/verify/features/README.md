# Shogi API verification map

Read the feature recipe before driving. This map is the maintained source of user paths, including paths the automated helper cannot currently reach.

## Baseline

Run `./scripts/verify-app.py FEATURE` from the repository root. Every invocation builds current sources, owns a dedicated Compose project and fresh data, runs doctor, drives the journey and cleans up. Native journeys launch game-server and player-server; `gateway` launches the public edge, gateway, example-only account service, game-server and real notification services. Evidence is printed under `target/verification/shogi-verify-<id>/`. No Google credentials or existing database are used. Install the browser dependency once as described in the skill before running `gateway`.

## Proof reporting

Pair commands/actions with read-back observations. Include feature ID, entry point, evidence directory and cleanup result. Preserve evidence after teardown. Report missing preconditions as a failed attempted journey with the reason; label untouched branches unverified. One entry point does not establish another. Do not treat integration tests or direct gRPC as browser proof.

## Features

- [Play a game](gameplay.md): matchmaking, promotion, capture, drop, resignation and exact retries through internal gRPC.
- [Resume after restart](persistence.md): in-progress and completed state, replaying original receipts.
- [Read public records](records.md): filtered HTTP search, detail, missing records and invalid queries.
- [Read committed ratings](ratings.md): initial state, worker application, separate rating pools.
- [Accounts and browser realtime](browser-realtime.md): automated verification-provider login, profile/logout, ticket lifecycle and authenticated WebSocket commands. Actual Google login is a separate path requiring real accounts.

Outbox failure/recovery also has an existing live harness, `scripts/prove-outbox.sh`, documented in the root README. It requires Valkey and uses its own Compose project; it is not run by `verify-app.py`. Natural clock timeout, full pagination, cancellation/expiry branches, spectating/access denial, and quarantined rating retry need additional journeys when those behaviors change. Never infer coverage for them from the four automated commands.
