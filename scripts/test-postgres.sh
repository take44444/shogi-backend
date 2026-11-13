#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ -n "${TEST_DATABASE_URL:-}" ] && [ -n "${TEST_VALKEY_URL:-}" ]; then
    if [ "$#" -eq 0 ]; then set -- cargo test --locked --workspace; fi
    exec "$@"
fi
project="shogi-tests-$$"
export POSTGRES_PORT="${POSTGRES_PORT:-55433}"
export VALKEY_PORT="${VALKEY_PORT:-56333}"
services=""
if [ -z "${TEST_DATABASE_URL:-}" ]; then services="$services postgres"; fi
if [ -z "${TEST_VALKEY_URL:-}" ]; then services="$services valkey"; fi
cleanup() {
    result=$?
    if [ "$result" -ne 0 ]; then docker compose -p "$project" logs --no-color $services; fi
    docker compose -p "$project" down --volumes --remove-orphans
    exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
docker compose -p "$project" up -d --wait $services
if [ -z "${TEST_DATABASE_URL:-}" ]; then
    export TEST_DATABASE_URL="postgres://postgres:postgres@127.0.0.1:$POSTGRES_PORT/postgres"
fi
if [ -z "${TEST_VALKEY_URL:-}" ]; then
    export TEST_VALKEY_URL="redis://127.0.0.1:$VALKEY_PORT"
fi
if [ "$#" -eq 0 ]; then set -- cargo test --locked --workspace; fi
"$@"
