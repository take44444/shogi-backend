#!/usr/bin/env python3
from pathlib import Path
import json
import subprocess
import sys
import tempfile
import unittest


CHECKER = Path(__file__).with_name("check-boundaries.py").resolve()
BASE = {
    "user-id": [],
    "shogi-domain": [],
    "rating-domain": ["shogi-domain"],
    "rating-worker-server": ["game-application", "game-persistence"],
    "user-application": ["user-id"],
    "user-persistence": ["user-application"],
    "user-http": ["user-application"],
    "user-server": ["user-http", "user-persistence"],
    "game-application": ["user-id", "shogi-domain", "rating-domain"],
    "game-record-application": ["user-id", "shogi-domain", "rating-domain"],
    "game-record-persistence": ["game-record-application", "shogi-domain"],
    "game-persistence": ["game-application"],
    "game-grpc": ["game-application"],
    "game-messaging": ["game-application"],
    "realtime-gateway": ["game-application", "game-grpc", "game-messaging"],
    "game-server": ["game-application", "game-grpc", "game-persistence"],
    "outbox-publisher": ["game-messaging", "game-persistence"],
    "realtime-gateway-server": ["realtime-gateway"],
    "player-http": ["game-record-application"],
    "player-server": ["game-record-persistence", "player-http"],
    "pg-test-support": [],
}


class Workspace:
    def __init__(self, root):
        self.root = Path(root)
        self.dependencies = {
            name: {"dependencies": {dependency: self.local(dependency) for dependency in dependencies}}
            for name, dependencies in BASE.items()
        }
        self.workspace_dependencies = {}
        self.binaries = {}
        self.write()

    def local(self, name, **options):
        entries = {"path": f"../{name}", **options}
        fields = ", ".join(f"{key} = {json.dumps(value)}" for key, value in entries.items())
        return "{ " + fields + " }"

    def add(self, source, name, value=None, section="dependencies"):
        self.dependencies[source].setdefault(section, {})[name] = value or self.local(name)
        self.write()

    def write(self):
        workspace = '[workspace]\nresolver = "3"\nmembers = ["crates/*"]\n'
        if self.workspace_dependencies:
            workspace += "\n[workspace.dependencies]\n"
            workspace += "".join(f"{name} = {value}\n" for name, value in self.workspace_dependencies.items())
        (self.root / "Cargo.toml").write_text(workspace)
        for name, sections in self.dependencies.items():
            directory = self.root / "crates" / name
            (directory / "src").mkdir(parents=True, exist_ok=True)
            (directory / "src" / "lib.rs").write_text("")
            manifest = f'[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2024"\n'
            if name.endswith("-server") or name == "outbox-publisher":
                (directory / "src" / "main.rs").write_text("fn main() {}")
            if name in self.binaries:
                manifest += f'\n[[bin]]\nname = "{self.binaries[name]}"\npath = "src/main.rs"\n'
            for section, dependencies in sections.items():
                manifest += f"\n[{section}]\n"
                manifest += "".join(f"{dependency} = {value}\n" for dependency, value in dependencies.items())
            (directory / "Cargo.toml").write_text(manifest)

    def run(self):
        return subprocess.run(
            [sys.executable, str(CHECKER), "--manifest-path", str(self.root / "Cargo.toml")],
            capture_output=True, text=True,
        )


class BoundaryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="boundary-policy-")
        self.addCleanup(self.temporary.cleanup)
        self.workspace = Workspace(self.temporary.name)

    def assert_rejected(self, *messages):
        result = self.workspace.run()
        self.assertEqual(result.returncode, 1, result.stderr)
        for message in messages:
            self.assertIn(message, result.stderr)

    def test_valid_production_graph(self):
        result = self.workspace.run()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_direct_writer_dependency(self):
        self.workspace.add("player-server", "game-persistence")
        self.assert_rejected("player-server -> game-persistence", "public reads must use game-record-persistence")

    def test_transitive_writer_dependency(self):
        self.workspace.add("game-record-persistence", "game-persistence")
        self.assert_rejected("player-server -> game-record-persistence -> game-persistence", "public reads must use game-record-persistence")

    def test_gateway_rejects_database_credentials_directly_or_transitively(self):
        self.workspace.add("realtime-gateway", "game-persistence")
        self.assert_rejected("realtime-gateway -> game-persistence", "gateway must use the user and game services")
        self.workspace.dependencies["realtime-gateway"]["dependencies"].pop("game-persistence")
        self.workspace.write()
        self.workspace.add("game-messaging", "game-persistence")
        self.assert_rejected("realtime-gateway -> game-messaging -> game-persistence", "gateway must use the user and game services")

    def test_renamed_dependency_uses_actual_package(self):
        self.workspace.add("player-http", "innocent", self.workspace.local("game-application", package="game-application"))
        self.assert_rejected("player-http -> game-application", "forbidden normal dependency")

    def test_optional_dependency_is_checked_without_enabling_it(self):
        self.workspace.add("game-application", "game-messaging", self.workspace.local("game-messaging", optional=True))
        self.assert_rejected("game-application -> game-messaging", "independent of the publisher and brokers")

    def test_inactive_target_alias_is_checked(self):
        self.workspace.add(
            "player-server", "hidden", self.workspace.local("user-persistence", package="user-persistence"),
            section="target.'cfg(target_os = \"windows\")'.dependencies",
        )
        self.assert_rejected("player-server -> user-persistence", 'cfg(target_os = "windows")', "share only user-id")

    def test_build_dependency_is_production(self):
        self.workspace.add("game-record-application", "game-persistence", section="build-dependencies")
        self.assert_rejected("game-record-application -> game-persistence", "forbidden build dependency")

    def test_target_build_dependency_is_production(self):
        self.workspace.add(
            "shogi-domain", "user-http", section="target.'cfg(target_os = \"windows\")'.build-dependencies",
        )
        self.assert_rejected("shogi-domain -> user-http", "forbidden build dependency")

    def test_workspace_inherited_alias_is_resolved(self):
        self.workspace.workspace_dependencies["hidden"] = '{ package = "game-application", path = "crates/game-application" }'
        self.workspace.add("player-http", "hidden", "{ workspace = true, optional = true }")
        self.assert_rejected("player-http -> game-application", "forbidden normal dependency")

    def test_unknown_workspace_package_fails_closed(self):
        self.workspace.dependencies["new-helper"] = {}
        self.workspace.write()
        self.assert_rejected("new-helper: unclassified workspace package", "assign its boundary role")

    def test_game_and_user_share_only_value_id(self):
        self.workspace.add("game-application", "user-application")
        self.assert_rejected("game-application -> user-application", "share only user-id")

    def test_rating_domain_cannot_acquire_writer_or_database_capabilities(self):
        self.workspace.add("rating-domain", "game-persistence")
        self.assert_rejected("rating-domain -> game-persistence", "forbidden normal dependency")

    def test_public_reader_cannot_import_rating_worker(self):
        self.workspace.add("player-server", "rating-worker-server")
        self.assert_rejected("player-server -> rating-worker-server", "forbidden normal dependency")

    def test_core_adapter_independence(self):
        self.workspace.add("shogi-domain", "game-grpc")
        self.assert_rejected("shogi-domain -> game-grpc", "domain/application must stay independent of adapters")

    def test_production_test_support_is_forbidden(self):
        self.workspace.add("game-persistence", "pg-test-support")
        self.assert_rejected("game-persistence -> pg-test-support", "test fixtures belong in dev-dependencies")

    def test_dev_fixture_does_not_pollute_production_graph(self):
        self.workspace.add("player-http", "game-persistence", section="dev-dependencies")
        self.workspace.add("player-http", "game-application", section="target.'cfg(unix)'.dev-dependencies")
        self.workspace.add("player-http", "pg-test-support", section="dev-dependencies")
        result = self.workspace.run()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_server_executable_uses_package_name(self):
        self.workspace.binaries["player-server"] = "postgres"
        self.workspace.write()
        self.assert_rejected("player-server: executable postgres violates server naming")

    def test_external_production_dependency_requires_review(self):
        self.workspace.add("game-application", "reqwest", '"0.12"')
        self.assert_rejected("game-application -> reqwest", "forbidden normal dependency")

    def test_package_name_cannot_disguise_a_local_path(self):
        self.workspace.add("game-application", "user-id", self.workspace.local("game-messaging", package="user-id"))
        self.assert_rejected("game-application -> user-id: local production dependency must be a classified workspace package")


if __name__ == "__main__":
    unittest.main()
