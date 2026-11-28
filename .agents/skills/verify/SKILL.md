---
name: verify
description: Verify shogi-backend through live gRPC games and public HTTP records and ratings. Use after API or persistence changes, for restart proofs, or to capture evidence of user journeys; consult the feature map for Google login and browser realtime prerequisites.
---

# Verify shogi-backend

Read [features/README.md](features/README.md), then the feature being checked. Run commands from the repository root. The primary surface is a backend API: internal gRPC for trusted game clients and public HTTP for records and ratings. Google login and browser HTTP/WebSocket are separate entry points requiring real accounts.

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

## Doctor

The helper runs this read-only check before every drive and after each game-server restart:

```sh
./scripts/verify-app.py doctor target/verification/shogi-verify-<id>
```

Replace the directory with the exact path printed by the active run. It checks the checkout, process executable and PID start token, ownership of the bound TCP ports, public OpenAPI identity, and a public database read. It prints the revision and endpoints. Run it first whenever a live instance looks wrong. After automatic cleanup it must fail because the processes are gone; inspect the saved `doctor.json` then. The helper builds current sources before launch; `worktree.json` captures tracked changes alongside the revision. Untracked source changes need separate evidence.

## Drive

```sh
./scripts/verify-app.py play
./scripts/verify-app.py persistence
./scripts/verify-app.py records
./scripts/verify-app.py ratings
```

Each invocation launches and cleans up its own stack. The gRPC clients are the repository's `play`, `persistence_proof`, and `rating_proof` examples, compiled with the application. HTTP reads use Python's standard library. See the map for each command's assertions and uncovered branches. `ratings` starts a real rating-worker-server after recording the initial ratings. Browser paths use [browser-realtime.md](features/browser-realtime.md); these commands do not verify authentication, WebSockets, Outbox delivery, or natural clock expiry.

## Evidence

The printed `target/verification/shogi-verify-<id>/` contains command arguments, combined stdout/stderr and exit codes, service and database logs, `instance.json`, `doctor.json`, revision/diff, and `result.json`. HTTP proofs record method, complete URL, status, headers, and body. Persistence adds protobuf snapshots and original receipts under `persistence/`.

Require a zero helper exit, `result.json` with `status: passed` and empty `cleanup_errors`, and the feature's expected observations. Verify the action and resulting state together: gRPC clients send actual matchmaking, moves and resignation, then read snapshots; HTTP detail/search cross-check persisted records; rating reads verify the worker's committed effects. Persistence compares snapshots and receipts across two real process restarts. A listening port or a build alone is not a proof. Use real APIs rather than setters or DB inserts. Mocks belong only at existing production boundaries; the supplied automated journeys use no mocks and no dry-run mode.

Record each feature ID and entry point in the verification report. A passed command covers only its mapped behaviors. Report unreachable paths as failed with their missing precondition; leave unattempted paths unverified. For browser proofs capture sanitized action/response transcripts and screenshots showing the origin; exclude cookies, OAuth callback query strings and credentials.

## Cleanup

The helper retains references to its own child processes, sends SIGINT, waits, and kills only a child that exceeds the shutdown deadline. It runs `docker compose down --volumes --remove-orphans` only for its UUID project. Evidence survives under `target/verification/`; retain or copy it before running `cargo clean`.

After an uncatchable interruption, read that run's `instance.json`. Confirm each live PID's `/proc/<pid>/stat` start token and `/proc/<pid>/exe` match before signaling it. Never kill by process name. To remove just that run's database, use the `project` value in its instance file:

```sh
docker compose --env-file /dev/null -f compose.yaml -p shogi-verify-<id> down --volumes --remove-orphans
```

Use the exact recorded project name. Check `docker ps -a --filter label=com.docker.compose.project=shogi-verify-<id>` and `docker volume ls --filter label=com.docker.compose.project=shogi-verify-<id>` return no resources. Confirm the evidence directory still contains `result.json` and the journey transcript. A cleanup error fails the run.

## Helpers

`scripts/verify-app.py` is executable; `./scripts/verify-app.py --help` lists the journeys and doctor. It is the only generated helper. The existing gRPC examples remain the source of game actions and assertions. Keep this skill and its map current with `/maintain-verification-skill` after routes, examples or startup requirements change.
