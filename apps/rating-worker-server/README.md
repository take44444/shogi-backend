# Run the rating worker

Start PostgreSQL, the game server and the independent worker with `docker compose up`.
Add `--profile records` to start the anonymous player HTTP server.

For local binaries, set `DATABASE_URL` to the same Game DB used by `game-server`.
Run `cargo run -p rating-worker-server`. The worker validates schema version 6
before polling and stops on Ctrl-C.

The worker uses these environment variables.

| Variable | Default | Meaning |
| --- | --- | --- |
| `DATABASE_URL` | `postgres://shogi:shogi@localhost:5432/shogi` | Game DB writer connection. |
| `RATING_WORKER_ID` | `worker-<process-id>` | Process identity, 1 through 128 bytes without whitespace. Use distinct names for concurrent workers. |
| `RATING_LEASE_MS` | `30000` | Positive claim lifetime in milliseconds. |
| `RATING_RETRY_LIMIT` | `5` | Positive maximum acquisitions, including expired claims. |
| `RATING_BACKOFF_MS` | `1000` | Delay before a transient retry. |
| `RATING_POLL_MS` | `250` | Positive delay between ticks. |

## Retry a quarantined game

Inspect `rating_jobs.last_error`, the immutable `rated_games` starts and the
corresponding committed game result with writer credentials. Repair corrupt
stored input before retrying. A retry does not repair input automatically.

Run the operator command with an explicit game ID.

```sh
cargo run -p rating-worker-server -- retry-quarantined GAME_ID
```

For Compose, run `docker compose run --rm rating-worker-server retry-quarantined GAME_ID`.
`Retried` means the job is pending with a fresh attempt budget and generation.
`NotQuarantined` and `NotFound` leave storage unchanged. Reservations remain
occupied throughout quarantine and retry. Do not delete reservations to recover
work, because admission uses them to freeze ordered starts.

## Exercise a real game

Start `game-server`, `rating-worker-server` and `player-server` against the same
database. Use a new prefix for each run.

```sh
cargo run -p game-grpc --example rating_proof -- http://127.0.0.1:50051 rating-demo
curl http://127.0.0.1:8081/users/rating-demo-black/ratings
```

The example creates a rated game and resigns White. Once the worker applies it,
Black has 1516 and White has 1484 in `ten_minute_30`, each with one completed
rated game. Their other pool remains null with zero games.
