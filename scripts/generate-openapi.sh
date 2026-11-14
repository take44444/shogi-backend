#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
output=${1:-target/openapi}
mkdir -p "$output"
for service in player user; do
  cargo run --quiet --locked -p "$service-http" --example "$service-openapi" > "$output/$service.json"
done
cargo run --quiet --locked -p realtime-gateway --example matchmaking-openapi > "$output/matchmaking.json"
