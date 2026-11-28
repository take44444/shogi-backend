# Resume after restart

A game survives a server restart, and retrying an already accepted command returns its original result after both in-progress and finished restarts.

## Sub-features

- `restart-ongoing`: restore the exact snapshot at sequence 3.
- `restart-retry`: recover the original first move receipt after restart.
- `restart-finished`: restore sequence 6 and both original move/resign receipts after a second restart.

## How to get to it (user POV)

Reconnect an internal gRPC client to the same server address after process restart, retaining game identity and pending request content. A separate container recreation procedure is documented in the root README.

## Driving it with verify-app.py

Preconditions: baseline setup; all stages must share one disposable database and proof directory.

- **Seed:** run `./scripts/verify-app.py persistence`. It invokes `persistence_proof seed`, joins two users, plays three moves and saves identity, initial receipt and sequence-3 snapshot as protobuf files.
- **Resume:** the helper stops its game-server with SIGINT and starts it again using the same DB and port. Doctor must pass. `persistence_proof resume` requires the exact saved snapshot and first receipt, plays two more moves and resigns at sequence 6.
- **Reconnect again:** the helper restarts game-server a second time. `persistence_proof finished` requires the exact final snapshot and both original receipts without further state changes.
- **Proof:** retain all three `persistence_proof-*.log` files, `doctor*.json`, and `persistence/{identity,first,ongoing,finished,resign}.bin`. The final transcript must include `finished restart verified exact snapshot and both original receipts at sequence 6`.

## Gotchas

- The helper proves graceful native process restart on the real PostgreSQL store. It does not establish container recreation, host failure, power loss or deployment migration behavior.
- Clocks continue during downtime. The ten-minute game must finish before its deadline; do not pause between stages.
- Persisted snapshot fields, including clock state and deadlines, must match exactly. Response-only `server_now_ms` must advance or remain equal; it is not persisted state. Receipts still match in full, including their original clock values.
- Deleting the DB volume between stages invalidates the journey. Cleanup deletes it only after the last stage or failure, preserving evidence files.
