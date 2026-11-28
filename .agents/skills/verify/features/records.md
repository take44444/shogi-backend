# Read public game records

An anonymous HTTP client finds a completed game by public filters and reads its ordered history.

## Sub-features

- `records-search`: a participant/winner/reason search includes the completed game.
- `records-detail`: detail agrees with search metadata and the public operation history.
- `records-missing`: a missing record returns 404.
- `records-invalid`: reversed time bounds return 400.

## How to get to it (user POV)

Call `GET /game-records` with required UTC-microsecond `from` and `to`, then `GET /game-records/{id}`. Discover schemas through `GET /openapi.json` on player-server. These routes require no login.

## Driving it with verify-app.py

Preconditions: baseline setup; a public completed game is required and is created through real gRPC in this journey.

- **Create a completed game:** run `./scripts/verify-app.py records`. `rating_proof` matches fresh users and has white resign. It records the actual game ID, black ID and white ID.
- **Search:** the helper sends `GET /game-records?from=<run-start>&to=<now>&participant=<black>&winner=black&reason=resignation&limit=1` with runtime values derived from the real action. Require exactly one item with the client's game ID.
- **Detail:** it requests that item's `/game-records/{id}`. Metadata must equal the search item; history must be exactly one white `resign` operation at sequence 1.
- **Negative paths:** it requests `/game-records/does-not-exist` (404) and `/game-records?from=1&to=0` (400).
- **Proof:** retain `rating_proof-*.log`, `search.json`, `detail.json`, `missing.json`, `invalid-search.json`, doctor and result. This cross-check proves the mutation is visible from a separate public read-only service.

## Gotchas

- `from` is inclusive, `to` exclusive and spans at most 366 days. Milliseconds are not accepted as a substitute for microseconds.
- A freshly created ongoing game is not a public completed record. Missing, ongoing and hidden records intentionally look alike through detail.
- The single-game fixture cannot prove pagination. Cursor traversal, other filters and hidden/ongoing distinctions require additional real games; report those branches unverified until driven.
