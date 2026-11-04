#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
project="shogi-outbox-proof-$$"
export GAME_SERVER_PORT="${GAME_SERVER_PORT:-55051}"
export POSTGRES_PORT="${POSTGRES_PORT:-55435}"
export VALKEY_PORT="${VALKEY_PORT:-56379}"
cleanup() {
    result=$?
    if [ "$result" -ne 0 ]; then docker compose -p "$project" logs --no-color; fi
    docker compose -p "$project" down --volumes --remove-orphans
    exit "$result"
}
trap cleanup EXIT INT TERM
docker compose -p "$project" up -d --build
cargo run --locked -p game-grpc --example outbox_proof -- \
    "http://127.0.0.1:$GAME_SERVER_PORT" "redis://127.0.0.1:$VALKEY_PORT" "$project"
