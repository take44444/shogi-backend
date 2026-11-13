# Realtime gateway

The gateway lets an authenticated browser watch one game per WebSocket connection. Participants can submit moves and resignations. Other signed-in users can watch public games. Private games are visible only to their participants.

Run the User Service, Game Service, Valkey, and outbox publisher first. The gateway has no database credentials. It calls the User Service at a fixed internal URL for every authentication check and the Game Service at a fixed internal gRPC endpoint for game commands and viewing rights.

```sh
GATEWAY_PUBLIC_ORIGIN=http://localhost:8082 \
GATEWAY_ALLOW_LOOPBACK_HTTP=true \
USER_SERVICE_URL=http://127.0.0.1:8080 \
GAME_SERVICE_URL=http://127.0.0.1:50051 \
VALKEY_URL=redis://127.0.0.1:6379 \
cargo run --locked -p realtime-gateway-server
```

The defaults are `GATEWAY_BIND_ADDR=127.0.0.1:8082`, `GATEWAY_MAX_CONNECTIONS=256`, `GATEWAY_MAX_USER_CONNECTIONS=8`, `GATEWAY_QUEUE_CAPACITY=64`, `GATEWAY_MAX_INPUT_BYTES=4096`, `GATEWAY_MAX_SNAPSHOT_BYTES=262144`, and `GATEWAY_MESSAGES_PER_SECOND=10`. Every limit can be set to a positive value with its named environment variable. The User Service must issue its development `user_session` cookie for the same host as the gateway. HTTPS uses `__Host-user_session` and does not require the loopback flag.

For a browser on one origin, use `docker compose --profile realtime up --build -d`. Its `browser-edge` serves authentication and `/ws/games/*` at `http://localhost:8082` by default. Set `REALTIME_PUBLIC_ORIGIN` and `REALTIME_PORT` together if the browser URL changes. Set `GOOGLE_CLIENT_ID` and `GOOGLE_CLIENT_SECRET` for login. The standalone `auth` profile has its own User Service origin; use the `realtime` profile alone for this flow. Production should terminate HTTPS at a same-origin reverse proxy and keep User Service, Game Service, gateway, and Valkey on an internal network.

## WebSocket contract

Open `GET /ws/games/{game_id}` on the browser's origin with subprotocol `shogi.realtime.v1`. An `Origin` matching the configured origin exactly and a unique valid session cookie are required. If `Sec-Fetch-Site` is present, it must be `same-origin`. A missing or inaccessible game returns HTTP 404 before upgrade. An invalid session returns 401. Dependency failures return 503.

All 64-bit sequence values are decimal JSON strings. A connection starts with one `snapshot` containing `game_id`, `seq`, `board`, `hands`, `turn`, `in_check`, `result`, `legal_moves`, `black_user_id`, and `white_user_id`. The board contains occupied squares with `file`, `rank`, `owner`, and `kind`. Hands contain `player`, `piece`, and `count` for all hand pieces. The result is `null` or `{ "winner": "black" | "white", "reason": "checkmate" | "resignation" }`.

Send one command at a time. The gateway rejects unknown fields, client supplied user IDs, malformed USI moves, and noncanonical sequence strings.

```json
{"type":"move","request_id":"client-1","expected_seq":"0","usi":"7g7f"}
{"type":"resign","request_id":"client-2","expected_seq":"1"}
```

An accepted command receives `{ "type": "accepted", "request_id": "client-1", "seq": "1", "in_check": false, "result": null }`. This acknowledges durable acceptance; it does not advance the browser's position. Apply `snapshot` and `event` messages to the position. An event has `event_id`, `game_id`, `seq`, `kind`, and either `usi`, `in_check`, `result` for a move or `result` for a finish. Internal request and user IDs are omitted from events. A checkmating move and its finish event share a sequence. An event can arrive before the command's acceptance reply.

An `error` includes a stable `code` and the `request_id` when the command was parsed. Codes are `invalid_input`, `busy`, `not_participant`, `request_conflict`, `stale_seq`, `rejected`, and `outcome_unknown`. `stale_seq` includes `actual_seq` when Game Service supplied it. Reconnect for a new snapshot after `stale_seq` or `resync_required`. An internal corruption error closes the socket without exposing details.

If an acceptance reply is lost or the code is `outcome_unknown`, resend the identical command with the original `request_id`, `expected_seq`, and USI move. Do not generate a new request ID. A disconnect does not cancel a command already enqueued by Game Service.

Reconnect with exponential backoff and random jitter after a socket close. The gateway subscribes before fetching the initial snapshot and buffers notifications during that fetch. Valkey Pub/Sub can still lose the last notification without a later event or disconnect to expose the gap. Reconnecting retrieves the current saved snapshot. No periodic position sync or anonymous spectator access is provided.

The gateway checks the session and current viewing rights before each outgoing message and immediately before a command. It does not run a periodic session timer. A quiet connection can remain open after logout until it next sends or receives application data. If the User Service or Game Service cannot complete a required check, the connection closes. Already started operations and sends can race with logout or a visibility change.

The subscriber uses one exact Valkey channel per connection with a bounded RESP3 push receiver. It closes on overflow, malformed messages, or disconnection rather than resuming a possibly incomplete stream. The 4096-byte notification check occurs after the Redis library parses a frame; keep Valkey on a trusted internal network.
