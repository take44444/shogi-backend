# Accounts and browser realtime

Players log in, manage their profile, match an opponent and play over an authenticated WebSocket on one browser origin. The automated journey substitutes only the external identity provider with fixed disposable Alice/Bob identities. Google integration is a separate entry path.

## Sub-features

- `browser-account`: browser-bound login, distinct persisted accounts, profile rename/read-back and logout.
- `browser-ticket`: authenticated join, heartbeat, current participation and readiness.
- `browser-game`: initial snapshots, accepted move, opponent notification, exact retry, reconnect and resignation.
- `browser-denied`: missing/revoked session and foreign-Origin provider/gateway requests are rejected.
- `browser-google`: actual Google login, token exchange and identity verification. Requires real credentials/accounts.

## How to get to it (user POV)

- Open `/auth/google/start`. The verification stack redirects to `/__verify/authorize`, where Alice and Bob are available. The normal callback creates the session and redirects to `/me`.
- The production stack redirects to Google instead. Its callback is still `/auth/google/callback`.
- Read/rename the profile through `GET /me` and `PATCH /me`; log out through `POST /auth/logout`.
- Use `GET /matchmaking/me`, `PUT`/`DELETE /matchmaking/tickets/{ticket_id}`, and `POST /matchmaking/tickets/{ticket_id}/heartbeat` or `/ready`.
- Connect `/ws/games/{game_id}` with subprotocol `shogi.realtime.v1` after the ticket reaches `playing`.

## Driving it with verify-app.py and Playwright

Preconditions:

- Linux, Docker Compose with `!override` support, Node.js/npm and Chromium's system dependencies.
- Run `npm ci --prefix tools/traceability-viewer-tests` and `./tools/traceability-viewer-tests/node_modules/.bin/playwright install chromium` once before the journey. Install dependencies before concurrent runs.
- Use a clean committed tree for proof tied to a revision. No Google credentials or existing accounts are required for the verification-provider path.

- **Launch/doctor:** run `./scripts/verify-app.py gateway`. It builds current images, launches a fresh project with a loopback public origin, initializes the account/game databases and waits for the public account API and gateway session denial. `./scripts/verify-app.py doctor DIRECTORY` diagnoses an active run using its recorded project and origin.
- **Login:** the driver opens two isolated browser contexts and requires anonymous `/me` and `/matchmaking/me` to return 401. Each context follows the normal start route, selects Alice or Bob in the actual verification page, and reaches `/me`. Require distinct application account IDs and the normal HttpOnly, SameSite=Lax session cookies. Cookie values are excluded from evidence.
- **Profile:** PATCH each display name, reload, and GET `/me` to require the same account ID and persisted new name. Restore names before logout.
- **Origin boundary:** while logged in, send a foreign-Origin request to the provider and to authenticated matchmaking. Both must be denied. After matching, a foreign-Origin WebSocket upgrade request for the real game must also be denied. These HTTP boundary probes retain denial status without cookie values.
- **Match:** join `ten_minute_30` using fresh ticket UUIDs, heartbeat, inspect current participation, and ready both tickets before their common deadline. Require the same game ID and `playing` before connecting WebSockets.
- **Move/retry:** both contexts receive sequence-0 snapshots. Black sends `7g7f`; require acceptance at sequence 1 and a `move_committed` notification at the opponent. Resend the identical request and compare the entire receipt. Reconnect both participants and require sequence 1 and matching changed boards.
- **Finish/logout:** white resigns at sequence 1. Require acceptance and final notifications at sequence 2, black's resignation win, and matching final snapshots with no legal moves. Capture actual profile response screenshots. Log out and require `/me`, matchmaking and WebSocket access to be denied.
- **Proof:** retain `browser-actions.json`, `browser.log`, `doctor.json`, screenshots, build/service/cleanup logs and `result.json` in the printed evidence directory. Require `status: passed`, empty `cleanup_errors`, and no remaining resources bearing the project's label. Screenshots supplement actual action/response/message observations; they are not a game UI.
- **Provider rejection tests:** run `POSTGRES_PORT=55443 VALKEY_PORT=56443 ./scripts/test-postgres.sh cargo test --locked -p user-http --example verification-user-server`. Retain stdout/stderr and the exit result. These Rust tests prove unknown actor, invalid/expired/replayed code, field binding, bounded temporary state, and a callback using another valid browser's login cookie. Expiry is tested at this seam, not by waiting in the browser journey. The example has `test = true`, so the normal workspace test command also runs it.
- **Real Google entry:** follow `apps/realtime-gateway/README.md` with two real test accounts, Google credentials and the registered public-origin callback. Login through the real Google page and repeat the documented HTTP/WebSocket operations. The verification-provider command does not establish this path.

## Gotchas

- The verification executable is a Cargo example in a separate image. Production user-server always uses Google and includes no verification-provider selector. A loopback HTTP public origin is required by the example.
- Temporary authorization requests/codes expire after 60 seconds and are lost on provider restart. Begin a fresh login if interrupted. Existing persisted sessions retain their normal behavior.
- All browser actions use the same exact origin; `localhost` and `127.0.0.1` do not share cookies. The gateway keeps its normal authentication and Origin checks.
- The notification observation requires the real publisher and Valkey. A receipt alone does not prove the opponent was notified.
- Identical command retries retain the original request ID and sequence. A new snapshot does not rewrite a pending request.
- Cancellation, natural clock timeout, spectator authorization, session expiry, queue overflow and Outbox failure/recovery remain additional journeys. Do not infer them from the automated command.
- A missing Google account is a failed precondition for an attempted Google journey. Keep the Google path unverified until real login succeeds.
