# Play a game

Two trusted API clients join matchmaking, confirm readiness, play shogi and obtain a final result.

## Sub-features

- `game-pair`: join, heartbeat and ready yield a playable initial board.
- `game-moves`: legal moves update the board, promotion and captured pieces in hand.
- `game-drop`: a captured bishop can be dropped and leaves the hand.
- `game-finish`: white resigns; black wins and no legal moves remain.
- `game-retry`: replayed move and resignation return the original receipts.

## How to get to it (user POV)

Use the trusted internal gRPC API described by `proto/game.proto`. Browser players enter via matchmaking HTTP and WebSocket; see [browser realtime](browser-realtime.md) for that distinct path.

## Driving it with verify-app.py

Preconditions: the baseline Linux/Rust/Docker setup; no application seed data is required.

- **Play:** run `./scripts/verify-app.py play`. The existing `play` client joins two fresh users in `ten_minute_30`, heartbeats and readies them. It requires sequence 0, 40 board pieces and 14 hand entries.
- **Move:** the same client sends `7g7f`, `3c3d`, `8h2b+`, `3a2b`, `B*4e` and reads a snapshot after each accepted move. Receipts advance through sequences 1–5. The transcript confirms promotion, recapture demotion and bishop drop.
- **Finish/retry:** white resigns at sequence 6; replaying resignation and the first move returns their original receipts. The final snapshot preserves board/hands and has no legal moves. Require the final log line `resignation and exact retries verified; black wins at sequence 6`.
- **Proof:** retain `play-*.log`, `doctor.json`, service logs and `result.json`. The client's read-backs are assertions against the live server, not fixture setters.

## Gotchas

- gRPC trusts user IDs and belongs on an internal network. Synthetic gRPC users do not establish browser accounts.
- The pair helper uses database time and a real matcher; do not replace it with direct game creation.
- This recipe covers the `ten_minute_30` pool and resignation. It does not establish timeout, checkmate, rejection cases or browser notifications.
