#!/usr/bin/env python3
"""Exercise the architecture checker through its command-line interface."""

from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


CHECKER = Path(__file__).with_name("check-architecture.py")
START = "<!-- architecture-inventory:start -->"
END = "<!-- architecture-inventory:end -->"


class ArchitectureCheckTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.put("scripts/check-architecture.py", CHECKER.read_text())
        self.put("Cargo.toml", '''[workspace]
members = ["crates/*", "apps/*"]
exclude = ["crates/excluded"]
[workspace.dependencies]
shared_alias = { package = "identity", path = "crates/identity" }
''')
        for directory, name in (
            ("crates/identity", "identity"),
            ("crates/user-http", "user-http"),
            ("crates/user-persistence", "user-persistence"),
            ("apps/server", "server"),
        ):
            self.put(f"{directory}/Cargo.toml", f'[package]\nname = "{name}"\nversion = "0.1.0"\n')
        self.put("crates/excluded/placeholder", "excluded non-package directory")
        self.put("crates/user-http/src/lib.rs", 'fn router() { Router::new().route("/me", get(me)); }\n')
        self.put("crates/user-persistence/src/lib.rs", '''const SCHEMA: &str = "CREATE TABLE accounts (id TEXT)";
#[cfg(test)]
mod tests { const SCRATCH: &str = "CREATE TABLE scratch (id TEXT)"; }
''')
        self.put("docs/design/reference.md", "Reference\n")
        self.document = self.root / "docs/design/architecture.md"
        self.document.write_text(f"# Architecture\n\n[Reference](reference.md)\n\n{START}\nstale\n{END}\n\nHandwritten prose.\n")

    def put(self, relative: str, contents: str) -> None:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents)

    def run_checker(self, *args: str, success: bool = True) -> subprocess.CompletedProcess[str]:
        result = subprocess.run(
            [sys.executable, str(self.root / "scripts/check-architecture.py"), *args],
            cwd=self.root.parent, text=True, capture_output=True, check=False,
        )
        self.assertEqual(result.returncode, 0 if success else 1, result.stdout + result.stderr)
        return result

    def refresh(self) -> None:
        self.run_checker("--write")
        self.run_checker("--check")

    def assert_stale(self, expected_fact: str) -> None:
        before = self.document.read_bytes()
        result = self.run_checker(success=False)
        self.assertIn("--write", result.stderr)
        self.assertIn("--- docs/design/architecture.md", result.stderr)
        self.assertIn(expected_fact, result.stderr)
        self.assertEqual(before, self.document.read_bytes())
        self.refresh()
        self.assertIn(expected_fact, self.document.read_text())

    def test_refresh_preserves_prose_and_is_idempotent(self) -> None:
        self.refresh()
        before = self.document.read_bytes()
        self.run_checker("--write")
        self.assertEqual(before, self.document.read_bytes())
        text = self.document.read_text()
        self.assertTrue(text.startswith("# Architecture\n\n[Reference](reference.md)\n\n" + START))
        self.assertTrue(text.endswith(END + "\n\nHandwritten prose.\n"))
        self.assertNotIn("scratch", text)
        self.assertNotIn("crates/excluded", text)

    def test_added_workspace_package_is_stale(self) -> None:
        self.refresh()
        self.put("apps/worker/Cargo.toml", '[package]\nname = "worker"\nversion = "0.1.0"\n')
        self.assert_stale("`worker`")

    def test_removed_workspace_package_can_be_refreshed(self) -> None:
        self.refresh()
        shutil.rmtree(self.root / "apps/server")
        self.run_checker(success=False)
        self.refresh()
        self.assertNotIn("`server`", self.document.read_text())

    def test_production_dependency_is_stale_and_uses_package_name(self) -> None:
        self.refresh()
        path = self.root / "apps/server/Cargo.toml"
        path.write_text(path.read_text() + '[dependencies]\nrenamed = { package = "identity", path = "../../crates/identity" }\n')
        self.assert_stale("| `identity` |")
        row = next(line for line in self.document.read_text().splitlines() if line.startswith("| `server` |"))
        self.assertTrue(row.endswith("| `identity` |"), row)
        self.assertNotIn("renamed", row)

    def test_workspace_and_target_build_dependencies_exclude_dev_dependencies(self) -> None:
        path = self.root / "apps/server/Cargo.toml"
        path.write_text(path.read_text() + '''[dependencies]
shared_alias.workspace = true
[target.'cfg(unix)'.build-dependencies]
http_alias = { package = "user-http", path = "../../crates/user-http" }
[dev-dependencies]
user-persistence = { path = "../../crates/user-persistence" }
''')
        self.refresh()
        row = next(line for line in self.document.read_text().splitlines() if line.startswith("| `server` |"))
        self.assertTrue(row.endswith("| `identity`, `user-http` |"), row)

    def test_added_route_is_stale(self) -> None:
        self.refresh()
        self.put("crates/user-http/src/lib.rs", 'fn router() { Router::new().route("/me", get(me)).route("/auth/logout", post(logout)); }')
        self.assert_stale("`/auth/logout`")

    def test_route_bound_registry_tracks_paths_and_multiple_methods(self) -> None:
        self.put("crates/user-http/src/lib.rs", '''fn api_router() -> OpenApiRouter<App> {
    OpenApiRouter::new().routes(routes!(me)).routes(routes!(rename))
}
#[utoipa::path(get, path = "/me", responses((status = 200, body = ProfileBody)))]
async fn me() {}
#[utoipa::path(patch, path = "/me", request_body = Rename, responses((status = 200)))]
async fn rename() {}
''')
        self.refresh()
        path = self.root / "crates/user-http/src/lib.rs"
        source = path.read_text().replace(".routes(routes!(rename))", ".routes(routes!(rename)).routes(routes!(logout))")
        source += '\n#[utoipa::path(post, path = "/auth/logout", responses((status = 204)))]\nasync fn logout() {}\n'
        path.write_text(source)
        self.assert_stale("`/auth/logout`")
        self.assertEqual(self.document.read_text().count("- `/me`"), 1)

    def test_unsupported_route_bound_registrations_fail_closed(self) -> None:
        valid = '''fn api_router() { OpenApiRouter::new().routes(routes!(me)) }
#[utoipa::path(get, path = "/me", responses((status = 200)))]
async fn me() {}
'''
        for source in (
            valid.replace("routes!(me)", "routes!(undocumented)"),
            valid.replace('path = "/me"', 'path = PATH'),
            valid.replace('path = "/me"', 'path = "/me", context_path = "/v2"'),
            valid.replace('path = "/me"', 'path = "/me", path = "/v2/me"'),
            valid.replace(".routes(routes!(me))", ".routes(other_routes())"),
            valid.replace(".routes(routes!(me))", ".routes(routes!(me, other))"),
            valid + 'fn added() { Router::new().route("/extra", get(extra)); }',
            valid + 'fn added() { Router::new().nest("/extra", other()); }',
            valid + 'fn added() { Router::new().merge(other()); }',
            valid.replace(".routes(routes!(me))", ".routes(routes!(me)).routes(routes!(me))"),
            valid + '\n#[utoipa::path(get, path = "/other", responses((status = 200)))]\nasync fn other() {}',
        ):
            with self.subTest(source=source):
                self.put("crates/user-http/src/lib.rs", source)
                before = self.document.read_bytes()
                result = self.run_checker("--write", success=False)
                self.assertIn("Cannot parse every HTTP route", result.stderr)
                self.assertEqual(before, self.document.read_bytes())

    def test_added_table_is_stale(self) -> None:
        self.refresh()
        path = self.root / "crates/user-persistence/src/lib.rs"
        path.write_text('const MORE: &str = "CREATE TABLE IF NOT EXISTS sessions (id TEXT)";\n' + path.read_text())
        self.assert_stale("`sessions`")

    def test_missing_required_inputs_fail_without_writing(self) -> None:
        self.refresh()
        for relative in ("Cargo.toml", "apps/server/Cargo.toml", "crates/user-http/src/lib.rs", "crates/user-persistence/src/lib.rs"):
            with self.subTest(relative=relative):
                path = self.root / relative
                saved = path.read_bytes()
                path.unlink()
                before = self.document.read_bytes()
                for args in ((), ("--write",)):
                    result = self.run_checker(*args, success=False)
                    self.assertIn("Architecture check failed", result.stderr)
                    self.assertEqual(before, self.document.read_bytes())
                path.write_bytes(saved)

    def test_unparseable_additional_routes_and_tables_fail_closed(self) -> None:
        self.refresh()
        for relative, added in (
            ("crates/user-http/src/lib.rs", "Router::new().route(PATH, get(me));"),
            ("crates/user-persistence/src/lib.rs", 'const MORE: &str = "CREATE TABLE [new_table] (id TEXT)";'),
        ):
            with self.subTest(relative=relative):
                path = self.root / relative
                before_source = path.read_text()
                path.write_text(added + "\n" + before_source)
                result = self.run_checker("--write", success=False)
                self.assertIn("Cannot parse every", result.stderr)
                path.write_text(before_source)

    def test_empty_required_source_fails_without_writing(self) -> None:
        self.refresh()
        before = self.document.read_bytes()
        self.put("crates/user-http/src/lib.rs", "\n")
        for args in ((), ("--write",)):
            result = self.run_checker(*args, success=False)
            self.assertIn("Required input is empty", result.stderr)
            self.assertEqual(before, self.document.read_bytes())

    def test_missing_duplicate_and_reversed_markers_fail_without_writing(self) -> None:
        for contents in ("No markers", START + START + END, END + START):
            with self.subTest(contents=contents):
                self.document.write_text(contents)
                for args in ((), ("--write",)):
                    self.run_checker(*args, success=False)
                    self.assertEqual(contents, self.document.read_text())

    def test_broken_handwritten_links_fail_in_both_modes(self) -> None:
        self.refresh()
        self.document.write_text(self.document.read_text() + "\n[Missing](missing.md)\n")
        for args in ((), ("--write",)):
            result = self.run_checker(*args, success=False)
            self.assertIn("Broken local links", result.stderr)
            self.assertIn("missing.md", result.stderr)


if __name__ == "__main__":
    unittest.main()
