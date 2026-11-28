# Read committed ratings

Clients read the latest committed rating in each pool after an independent worker processes a completed rated game.

## Sub-features

- `ratings-initial`: fresh user IDs have null ratings and zero completed games in both pools.
- `ratings-apply`: one game produces one committed result per player, with the winner rated above the loser.
- `ratings-pools`: an unused pool retains its initial state.

## How to get to it (user POV)

Call unauthenticated `GET /users/{user_id}/ratings` on player-server, before and after a rated match. Both pools are returned together.

## Driving it with verify-app.py

Preconditions: baseline setup; the helper launches its own rating worker only after creating a completed game.

- **Initial state:** run `./scripts/verify-app.py ratings`. Before matchmaking it reads the two fresh users' ratings and requires null/zero for both pools.
- **Complete a match:** `rating_proof` uses `ten_minute_30`, matches users, confirms readiness and resigns white. Capture the client's actual sides and game ID.
- **Observe asynchronous application:** the helper starts rating-worker-server against the same DB and polls public HTTP for at most 60 seconds. Both `ten_minute_30.completed_rated_games` must become exactly 1, with black's rating greater than white's after equal initial ratings. Both `byoyomi_30` objects must remain null/zero.
- **Proof:** retain `before-{one,two}.json`, `after-{black,white}.json`, client transcript, worker log, doctor and result. The final HTTP reads prove committed effects beyond a worker's startup log.

## Gotchas

- Public ratings can lag a finished game. Poll for the result rather than asserting immediately.
- Unknown user IDs return null/zero; this API does not assert account existence.
- The helper proves one application per player, pool separation and relative ordering. It does not yet prove numeric deltas against a pre-existing rating, duplicate-job delivery, or quarantined retry. Those need their own fixtures and observations.
