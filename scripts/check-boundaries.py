#!/usr/bin/env python3
import argparse
from collections import deque
import json
from pathlib import Path
import subprocess
import sys


PACKAGES = {
    "user-id": ("value", "shared"),
    "shogi-domain": ("domain", "game"),
    "rating-domain": ("domain", "game"),
    "user-application": ("application", "user"),
    "game-application": ("application", "game"),
    "game-record-application": ("application", "game"),
    "user-persistence": ("persistence", "user"),
    "game-persistence": ("persistence", "game"),
    "game-record-persistence": ("persistence", "game"),
    "user-http": ("protocol", "user"),
    "player-http": ("protocol", "game"),
    "game-grpc": ("protocol", "game"),
    "game-messaging": ("messaging", "game"),
    "realtime-gateway": ("protocol", "gateway"),
    "user-server": ("server", "user"),
    "game-server": ("server", "game"),
    "rating-worker-server": ("server", "game"),
    "player-server": ("server", "game"),
    "outbox-publisher": ("publisher", "game"),
    "realtime-gateway-server": ("server", "gateway"),
    "pg-test-support": ("fixture", "test"),
}

DEPENDENCIES = {
    "user-id": set(),
    "shogi-domain": {"rsshogi"},
    "rating-domain": {"shogi-domain"},
    "rating-worker-server": {"game-application", "game-persistence", "tokio"},
    "user-application": {"user-id", "async-trait", "thiserror"},
    "game-application": {"user-id", "rating-domain", "shogi-domain", "tokio", "async-trait"},
    "game-record-application": {
        "user-id", "rating-domain", "shogi-domain", "async-trait", "base64", "serde_json", "sha2",
    },
    "user-persistence": {"user-application", "async-trait", "sea-orm", "uuid"},
    "game-persistence": {
        "game-application", "game-record-application", "rating-domain", "shogi-domain", "async-trait",
        "sea-orm", "sea-query-sqlx", "uuid",
    },
    "game-record-persistence": {
        "game-record-application", "shogi-domain", "async-trait", "sea-orm",
        "sea-query-sqlx",
    },
    "user-http": {
        "user-application", "async-trait", "axum", "utoipa", "utoipa-axum",
        "openidconnect", "serde", "sha2", "hex", "uuid", "url", "thiserror",
    },
    "player-http": {
        "game-record-application", "axum", "utoipa", "utoipa-axum", "serde",
        "serde_json", "tokio",
    },
    "game-grpc": {
        "game-application", "shogi-domain", "tonic", "tonic-prost", "prost",
        "tonic-prost-build", "protoc-bin-vendored",
    },
    "game-messaging": {
        "game-application", "shogi-domain", "async-trait", "redis", "serde",
        "serde_json", "tokio", "thiserror", "url",
    },
    "realtime-gateway": {
        "axum", "futures-util", "game-application", "game-grpc", "game-messaging",
        "shogi-domain", "reqwest", "serde", "serde_json", "tokio", "tonic", "url",
    },
    "user-server": {"user-application", "user-persistence", "user-http", "tokio", "axum"},
    "game-server": {"game-application", "game-persistence", "game-grpc", "tokio", "tonic"},
    "player-server": {
        "game-record-application", "game-record-persistence", "player-http", "tokio", "axum",
    },
    "outbox-publisher": {"game-application", "game-persistence", "game-messaging", "tokio"},
    "realtime-gateway-server": {"realtime-gateway", "tokio", "axum"},
    "pg-test-support": {"sqlx", "tokio", "uuid", "url"},
}

PUBLIC_READERS = {
    "game-record-application", "game-record-persistence", "player-http", "player-server",
}
PUBLIC_READ_FORBIDDEN = {
    "game-application", "game-persistence", "game-grpc", "game-messaging",
    "outbox-publisher", "user-application", "user-persistence", "user-http", "user-server",
    "openidconnect", "oauth2", "reqwest", "redis",
}
ACCEPTANCE = {"game-application", "game-persistence", "game-grpc", "game-server"}
BROKERS = {"game-messaging", "outbox-publisher", "redis", "lapin", "rdkafka", "async-nats"}
GATEWAY = {"realtime-gateway", "realtime-gateway-server"}
DATABASE_CAPABILITIES = {"game-persistence", "user-persistence", "game-record-persistence", "sea-orm", "sqlx"}


def reachable(graph, start):
    paths = {start: [start]}
    pending = deque([start])
    while pending:
        origin = pending.popleft()
        for dependency in sorted(graph.get(origin, ())):
            if dependency not in paths:
                paths[dependency] = [*paths[origin], dependency]
                pending.append(dependency)
    return paths


def check(metadata):
    members = set(metadata["workspace_members"])
    packages = {package["name"]: package for package in metadata["packages"] if package["id"] in members}
    directories = {
        Path(package["manifest_path"]).resolve().parent: name for name, package in packages.items()
    }
    errors = []
    graph = {}
    for name, package in sorted(packages.items()):
        if name not in PACKAGES or name not in DEPENDENCIES:
            errors.append(f"{name}: unclassified workspace package; assign its boundary role and dependency allowlist")
            continue
        layer, _ = PACKAGES[name]
        for target in package["targets"]:
            if "bin" not in target["kind"]:
                continue
            if layer == "server" and target["name"] == name and name.endswith("-server"):
                continue
            if layer == "publisher" and target["name"] == "outbox-publisher":
                continue
            errors.append(f"{name}: executable {target['name']} violates server naming; use the package's -server name or the outbox-publisher exception")
        graph[name] = set()
        for dependency in package["dependencies"]:
            if dependency["kind"] == "dev":
                continue
            actual = dependency["name"]
            graph[name].add(actual)
            if actual in PACKAGES or dependency.get("path"):
                local = directories.get(Path(dependency["path"]).resolve()) if dependency.get("path") else None
                if local != actual:
                    errors.append(f"{name} -> {actual}: local production dependency must be a classified workspace package at its declared path")
            if actual not in DEPENDENCIES[name]:
                kind = dependency["kind"] or "normal"
                target = f" for {dependency['target']}" if dependency.get("target") else ""
                errors.append(f"{name} -> {actual}: forbidden {kind} dependency{target}; remove it or explicitly review this package's dependency allowlist")

    for name in sorted(packages.keys() & PACKAGES.keys()):
        layer, context = PACKAGES[name]
        for dependency, path in reachable(graph, name).items():
            if dependency == name:
                continue
            target_layer, target_context = PACKAGES.get(dependency, (None, None))
            reason = None
            if target_layer == "fixture" and layer != "fixture":
                reason = "test fixtures belong in dev-dependencies"
            elif context in {"game", "user"} and target_context in {"game", "user"} and context != target_context:
                reason = "game and user contexts share only user-id"
            elif layer in {"value", "domain", "application"} and target_layer not in {None, "value", "domain", "application"}:
                reason = "domain/application must stay independent of adapters and composition"
            elif name in PUBLIC_READERS and dependency in PUBLIC_READ_FORBIDDEN:
                reason = "public reads must use game-record-persistence without writer, actor, broker or user-auth capabilities"
            elif name in ACCEPTANCE and dependency in BROKERS:
                reason = "game acceptance must remain independent of the publisher and brokers"
            elif name in GATEWAY and dependency in DATABASE_CAPABILITIES:
                reason = "gateway must use the user and game services without database credentials"
            if reason:
                errors.append(f"{' -> '.join(path)}: {reason}; remove the production dependency")
    return sorted(set(errors))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest-path", type=Path, default=Path(__file__).resolve().parents[1] / "Cargo.toml")
    args = parser.parse_args()
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps", "--all-features", "--locked",
         "--manifest-path", str(args.manifest_path)],
        capture_output=True, text=True,
    )
    if result.returncode:
        print(result.stderr.strip(), file=sys.stderr)
        return result.returncode
    errors = check(json.loads(result.stdout))
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Workspace production dependency policy passed (workspace paths and direct external allowlists; normal/build, all targets and optional declarations; dev fixtures excluded).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
