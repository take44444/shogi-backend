#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
project="shogi-outbox-proof-$$"
export GAME_SERVER_PORT="${GAME_SERVER_PORT:-55051}"
export NATS_PORT="${NATS_PORT:-54222}"
cleanup() {
    result=$?
    if [ "$result" -ne 0 ]; then docker compose -p "$project" logs --no-color; fi
    docker compose -p "$project" down --volumes --remove-orphans
    exit "$result"
}
trap cleanup EXIT INT TERM
docker compose -p "$project" up -d --build
cargo run --locked -p rpc --example outbox_proof -- \
    "http://127.0.0.1:$GAME_SERVER_PORT" "nats://127.0.0.1:$NATS_PORT" "$project"
