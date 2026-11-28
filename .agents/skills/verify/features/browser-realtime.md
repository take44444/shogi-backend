# Accounts and browser realtime

Players log in with Google, manage their profile, match an opponent and play over an authenticated WebSocket on one browser origin.

## Sub-features

- `browser-account`: Google login, persisted profile rename and logout.
- `browser-ticket`: join, heartbeat, current participation, readiness and cancellation.
- `browser-game`: initial snapshot, accepted move, opponent notification and reconnect.

## How to get to it (user POV)

- Open `/auth/google/start`; Google returns to `/auth/google/callback`, then `/me`.
- Read or rename through `GET /me` and `PATCH /me`; log out with `POST /auth/logout`.
- Use `GET /matchmaking/me`, `PUT`/`DELETE /matchmaking/tickets/{ticket_id}`, and `POST /matchmaking/tickets/{ticket_id}/heartbeat` or `/ready`.
- Connect `/ws/games/{game_id}` with subprotocol `shogi.realtime.v1` after the ticket is `playing`.

## Driving it with browser fetch and WebSocket

Preconditions: two real test accounts in separate browser profiles; Google web OAuth credentials; a registered callback URI; browser console access. The automated helper supplies none of these. A missing credential/account/browser is a failed precondition for an attempted browser journey, not a passing gRPC substitute.

- **Launch:** follow `apps/realtime-gateway/README.md` for Google credential setup. Set `export VERIFY_BROWSER_PROJECT="shogi-browser-$(python3 -c 'import uuid; print(uuid.uuid4().hex[:12])')"` and `export REALTIME_PORT=58082 REALTIME_PUBLIC_ORIGIN=http://localhost:58082 POSTGRES_PORT=55439 VALKEY_PORT=56389 GAME_SERVER_PORT=55059`. Confirm these ports are unused with `ss -ltn`; choose other unused values if needed. Register `http://localhost:58082/auth/google/callback` for this origin. Run `docker compose -p "$VERIFY_BROWSER_PROJECT" --profile realtime up --build -d`. Keep that exact project/environment for cleanup. Unlike `verify-app.py`, this branch reads configured `.env` credentials. Do not print them into evidence.
- **Login/doctor:** in each browser profile open the configured origin's `/auth/google/start`, complete Google login and require `/me` to return the intended account. At that origin run `await (await fetch('/me')).json()` and `await (await fetch('/matchmaking/me')).json()`. Require the expected account and initial `idle` participation; record sanitized response bodies and origin. Compose health alone does not prove valid auth.
- **Rename:** retain the previous `display_name`; run `await (await fetch('/me', {method:'PATCH', headers:{'Content-Type':'application/json'}, body:JSON.stringify({display_name:'Verification player'})})).json()`. A fresh `GET /me` must return the new name. Restore the previous name using the same PATCH before logout.
- **Join:** in each profile set `const ticket = crypto.randomUUID(); const path = '/matchmaking/tickets/' + ticket;` and retain it before sending. Run `await (await fetch(path, {method:'PUT', headers:{'Content-Type':'application/json'}, body:JSON.stringify({pool:'ten_minute_30'})})).json()`. Repeat uncertain requests with the same ticket and body.
- **Heartbeat/ready:** start with `let seq = 1`; send `await (await fetch(path+'/heartbeat', {method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify({sequence:seq})})).json()`. Increase `seq` only after its response. Repeat roughly every 5 seconds while waiting. On `matched` in each profile, send `await (await fetch(path+'/ready', {method:'POST'})).json()` before the 15-second deadline. Poll until `playing`; read `GET /matchmaking/me` to obtain the ticket's `game_id` and `side` after reconnect. A matched reservation is not yet playable.
- **Play:** assign `const current = await (await fetch('/matchmaking/me')).json(); const game = current.ticket.game_id; const socket = new WebSocket((location.protocol==='https:'?'wss:':'ws:')+'//'+location.host+'/ws/games/'+encodeURIComponent(game), 'shogi.realtime.v1'); socket.onmessage = e => console.log(JSON.parse(e.data));`. Require a `snapshot` in both profiles. In the black profile retain `const pending = {type:'move', request_id:crypto.randomUUID(), expected_seq:'0', usi:'7g7f'};` then send `socket.send(JSON.stringify(pending))`. Capture acceptance, the opponent's update and its resulting position. Resend the identical pending command; require the original receipt and no second board transition. Close/reopen the socket to require the updated snapshot. Finish by sending a `resign` command from white with a fresh request ID and the current snapshot's sequence string; require black's final win in both profiles.
- **Cancel:** after rating completion allows a new admission, join using a new UUID and run `await (await fetch(path, {method:'DELETE'})).json()` for that new ticket; confirm terminal cancellation and current participation. Use a fresh console binding for the new `path`.
- **Logout/proof:** `await fetch('/auth/logout', {method:'POST'})` must return 204; a subsequent `/me` returns 401. Capture sanitized requests/responses, WebSocket messages and screenshots to `target/verification/<browser-project>/`. Preserve the browser origin and action order. Close sockets and clear heartbeat timers, restore profile names, then run `docker compose -p "$VERIFY_BROWSER_PROJECT" --profile realtime down --volumes --remove-orphans` with the exact launch environment; retain evidence.

## Gotchas

- This recipe needs operator-provided accounts, credentials and browser access. It is mapped but was not proved when generating the automated skill. Do not create sessions by DB inserts or bypass Google authentication.
- Use exactly one origin throughout. `localhost` and `127.0.0.1` do not share cookies. Same-origin browser fetch supplies the Origin required for mutations.
- Logins, waiting tickets and games mutate real Google-backed test accounts. Use only dedicated verification accounts and their isolated stack.
- An exact move retry retains its original `expected_seq`; reconnect's new snapshot sequence does not rewrite that command.
- Natural clock timeout, forbidden spectators, session expiry, queue overflow and Outbox recovery remain separate unverified branches. The documented Outbox harness can supplement notification proofs.
