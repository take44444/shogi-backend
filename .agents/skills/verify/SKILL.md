---
name: verify
description: Verify shogi-backend through live gRPC, public records and ratings, or authenticated browser gateway journeys using isolated verification accounts. Use after API, persistence or gateway changes and for restart proofs; real Google login remains a separate integration check.
---

# Verify shogi-backend

Read [features/README.md](features/README.md), then the feature being checked. Run commands from the repository root. The primary surface is a backend API with trusted internal gRPC, public HTTP reads, and authenticated browser HTTP/WebSocket. The gateway journey substitutes the external identity provider while using real account/session persistence and game services. Real Google login requires real accounts and is a separate integration check.

## Launch

Prerequisites: Linux with `/proc`, Python 3.9+, the Rust toolchain pinned by `rust-toolchain.toml`, Cargo, Docker daemon, and Docker Compose v2. Registry access is needed when dependencies or `postgres:18` are uncached. No Google credentials are needed for the four automated journeys.

```sh
./scripts/verify-app.py play
```

This command owns the whole lifecycle: locked Cargo build into `target/debug`, dedicated PostgreSQL, migrations through game-server startup, player-server startup, doctor, live gRPC drive, evidence capture, and cleanup. The equivalent build command is:

```sh
cargo build --locked --target-dir target -p game-server -p player-server -p rating-worker-server -p game-grpc --bins --examples
```

The helper prints `Evidence: /.../target/verification/shogi-verify-<id>` before building. Each run uses a UUID Compose project, a fresh named database volume, an automatically assigned PostgreSQL port, and ephemeral loopback API ports. It invokes root `compose.yaml` with `--env-file /dev/null`, enables no profiles, and starts only PostgreSQL. It never loads `.env`. The native services receive disposable DB URLs and explicit bind addresses. Initial users and games are created through matchmaking gRPC, without DB seed writes.

Usable means application-role TCP login succeeds after init SQL, game-server owns its listener after schema initialization, player-server owns its listener, and a public record query succeeds with reader grants. A `pg_isready` success alone is insufficient. Expect `game-server listening on 127.0.0.1:<port>` and `player-server listening on 127.0.0.1:<port>` in retained logs. The helper automatically tears down after the journey, including failures and normal SIGINT/SIGTERM.

Concurrent instances have separate DBs, ports, and identities. Cargo builds share `target` and may wait for its lock. If an ephemeral port is claimed before bind, the run fails and cleans up; rerun with a new instance. Use no pre-existing app or database for automated journeys.

For the browser gateway journey, install the existing repository browser dependency and Chromium once:

```sh
npm ci --prefix tools/traceability-viewer-tests
./tools/traceability-viewer-tests/node_modules/.bin/playwright install chromium
./scripts/verify-app.py gateway
```

This branch requires Node.js, npm, Chromium's system dependencies, and Docker Compose with `!override` support. It builds current production services plus the separate `deploy/verification.Dockerfile`, then launches a UUID Compose project with `compose.verification.yaml`. Its public origin is an unused HTTP loopback port. Alice and Bob log in through the ordinary start/callback handlers using the example-only provider. The user service receives only verification configuration, with no Google credentials. All browser game actions go through the real public edge. Normal production images contain no verification provider. Docker Buildx may need execution permission to update its own files under `~/.docker`.

## Doctor

The helper runs this read-only check before every drive and after each game-server restart:

```sh
./scripts/verify-app.py doctor target/verification/shogi-verify-<id>
```

Replace the directory with the exact path printed by the active run. It checks the checkout, process executable and PID start token, ownership of the bound TCP ports, public OpenAPI identity, and a public database read. It prints the revision and endpoints. Run it first whenever a live instance looks wrong. After automatic cleanup it must fail because the processes are gone; inspect the saved `doctor.json` then. The helper builds current sources before launch; `worktree.json` captures tracked changes alongside the revision. Untracked source changes need separate evidence.

For `gateway`, doctor checks the recorded Compose project's running services, ownership of the published edge port, the account OpenAPI, and anonymous account/gateway denial. It uses the recorded origin, not shell-supplied Google settings. It also fails after cleanup.

## Drive

```sh
./scripts/verify-app.py play
./scripts/verify-app.py persistence
./scripts/verify-app.py records
./scripts/verify-app.py ratings
./scripts/verify-app.py gateway
```

Each invocation launches and cleans up its own stack. The gRPC clients are the repository's `play`, `persistence_proof`, and `rating_proof` examples, compiled with the application. HTTP reads use Python's standard library. `ratings` starts a real rating-worker-server after recording the initial ratings. `gateway` uses two Playwright browser contexts and the real publisher/Valkey notification path. See [browser-realtime.md](features/browser-realtime.md) for that journey and the separate Google path. Natural clock expiry and Outbox failure/recovery remain separate checks.

## Evidence

The printed `target/verification/shogi-verify-<id>/` contains command arguments, combined stdout/stderr and exit codes, service and database logs, `instance.json`, `doctor.json`, revision/diff, and `result.json`. HTTP proofs record method, complete URL, status, headers, and body. Persistence adds protobuf snapshots and original receipts under `persistence/`.

The gateway branch records sanitized HTTP requests/responses and WebSocket actions/messages in `browser-actions.json`, login-choice screenshots and actual `/me` response screenshots. It records no cookie values, HAR, browser traces or callback query strings. These API-page screenshots supplement the action transcript; the transcript proves the game state. Prefer a clean committed tree for attributable proof; a HEAD hash with untracked file names alone does not identify the code that ran.

Require a zero helper exit, `result.json` with `status: passed` and empty `cleanup_errors`, and the feature's expected observations. Verify the action and resulting state together: gRPC clients send actual matchmaking, moves and resignation, then read snapshots; HTTP detail/search cross-check persisted records; rating reads verify the worker's committed effects. Persistence compares snapshots and receipts across two real process restarts. A listening port or a build alone is not a proof. Use real APIs rather than setters or DB inserts. Mocks belong only at existing production boundaries; the supplied automated journeys use no mocks and no dry-run mode.

Record each feature ID and entry point in the verification report. A passed command covers only its mapped behaviors. Report unreachable paths as failed with their missing precondition; leave unattempted paths unverified. For browser proofs capture sanitized action/response transcripts and screenshots showing the origin; exclude cookies, OAuth callback query strings and credentials.

## Cleanup

The helper retains references to its own child processes, sends SIGINT, waits, and kills only a child that exceeds the shutdown deadline. It runs `docker compose down --volumes --remove-orphans` only for its UUID project. Gateway cleanup additionally removes that project's locally tagged images and checks that no labeled containers, volumes or networks remain. Evidence survives under `target/verification/`; retain or copy it before running `cargo clean`.

After an uncatchable interruption, read that run's `instance.json`. Confirm each live PID's `/proc/<pid>/stat` start token and `/proc/<pid>/exe` match before signaling it. Never kill by process name. To remove just that run's database, use the `project` value in its instance file:

```sh
docker compose --env-file /dev/null -f compose.yaml -p shogi-verify-<id> down --volumes --remove-orphans
```

Use the exact recorded project name. Check `docker ps -a --filter label=com.docker.compose.project=shogi-verify-<id>` and `docker volume ls --filter label=com.docker.compose.project=shogi-verify-<id>` return no resources. Confirm the evidence directory still contains `result.json` and the journey transcript. A cleanup error fails the run.

## Helpers

`scripts/verify-app.py` is executable; `./scripts/verify-app.py --help` lists the journeys and doctor. It invokes `node scripts/verify-gateway.cjs ORIGIN EVIDENCE_DIRECTORY` for the gateway branch. Use the complete launcher so the browser script only drives its own disposable instance. The existing gRPC examples remain the source of internal game actions and assertions. Keep this skill and its map current with `/maintain-verification-skill` after routes, examples or startup requirements change.
