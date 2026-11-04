#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ -n "${TEST_DATABASE_URL:-}" ]; then
    if [ "$#" -eq 0 ]; then set -- cargo test --locked --workspace; fi
    exec "$@"
fi
project="shogi-tests-$$"
export POSTGRES_PORT="${POSTGRES_PORT:-55433}"
cleanup() {
    result=$?
    if [ "$result" -ne 0 ]; then docker compose -p "$project" logs --no-color postgres; fi
    docker compose -p "$project" down --volumes --remove-orphans
    exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
docker compose -p "$project" up -d --wait postgres
export TEST_DATABASE_URL="postgres://postgres:postgres@127.0.0.1:$POSTGRES_PORT/postgres"
if [ "$#" -eq 0 ]; then set -- cargo test --locked --workspace; fi
"$@"
