#!/usr/bin/env python3
import json
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

import traceability_viewer as viewer


class ViewerTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        (self.root / "src").mkdir()
        self.source = self.root / "src/lib.rs"
        self.source.write_text('pub fn api() {}\n// </script><script>alert("source")</script>\n')
        self.api = {"id": "sample/api", "package": "sample", "name": "api", "kind": "function", "path": "src/lib.rs", "line": 1}
        self.bundle = {"version": 3, "components": [{"id": "sample", "package": "sample", "path": "."}],
                       "apis": [self.api], "calls": [], "tests": [], "unresolved": [], "coverage": {},
                       "units": [{"package": "sample", "target_kind": "lib", "target_name": "sample", "is_test": False, "features": [], "purposes": ["production"], "source": "src/lib.rs"}],
                       "excluded_targets": [], "configuration": {"scope": "Default features on current host", "toolchain": "nightly", "cargo": ["build", "test"]}}

    def git(self, *args):
        result = subprocess.run(["git", "-C", str(self.root), *args], capture_output=True, text=True, check=True)
        return result.stdout.strip()

    def init_git(self):
        self.git("init", "-q")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "user.name", "Fixture")
        self.git("add", ".")
        self.git("commit", "-qm", "fixture")
        self.git("remote", "add", "origin", "git@github.com:example/sample.git")

    def test_snapshot_embeds_compiler_span_source_without_needles(self):
        payload = viewer.capture(self.root, self.bundle)
        self.assertEqual(payload["version"], 3)
        self.assertEqual(payload["files"]["src/lib.rs"]["content"], self.source.read_text())
        self.assertNotIn("locations", payload)
        self.assertIsNone(payload["files"]["src/lib.rs"]["github_url"])

    def test_untrusted_source_cannot_escape_json_script(self):
        destination = self.root / "target/viewer.html"
        viewer.write_viewer(self.root, self.bundle, destination)
        html = destination.read_text()
        embedded = re.search(r'<script type="application/json" id="traceability-data">(.*?)</script>', html, re.DOTALL)[1]
        self.assertNotIn("<", embedded)
        self.assertNotIn("</script><script>alert", html)
        self.assertEqual(json.loads(embedded)["files"]["src/lib.rs"]["content"], self.source.read_text())
        self.assertNotIn("/* VIEWER_JS */", html)
        self.assertNotIn("<!-- VIEWER_DATA -->", html)

    def test_refuses_source_tracked_files_git_metadata_and_tool_assets(self):
        self.init_git()
        tracked = self.root / "tracked.txt"
        tracked.write_text("keep")
        self.git("add", "tracked.txt")
        for path in (self.source, tracked, self.root / ".git/config", viewer.ASSETS / "index.html"):
            with self.subTest(path=path):
                before = path.read_bytes()
                with self.assertRaisesRegex(ValueError, "refusing to overwrite"):
                    viewer.write_viewer(self.root, self.bundle, path)
                self.assertEqual(path.read_bytes(), before)

    def test_refuses_symlink_destination_to_source(self):
        link = self.root / "output.html"
        link.symlink_to(self.source)
        with self.assertRaisesRegex(ValueError, "refusing to overwrite"):
            viewer.write_viewer(self.root, self.bundle, link)
        self.assertTrue(link.is_symlink())

    def test_untracked_source_outside_snapshot_cannot_be_overwritten(self):
        destination = self.root / "src/untracked.rs"
        destination.write_text("fn keep() {}\n")
        with self.assertRaisesRegex(ValueError, "refusing to overwrite"):
            viewer.write_viewer(self.root, self.bundle, destination)
        self.assertEqual(destination.read_text(), "fn keep() {}\n")

    def test_github_links_require_byte_identical_committed_source(self):
        self.init_git()
        payload = viewer.capture(self.root, self.bundle)
        revision = self.git("rev-parse", "HEAD")
        self.assertEqual(payload["files"]["src/lib.rs"]["github_url"], f"https://github.com/example/sample/blob/{revision}/src/lib.rs")
        self.source.write_text(self.source.read_text() + "// local change\n")
        payload = viewer.capture(self.root, self.bundle)
        self.assertIsNone(payload["files"]["src/lib.rs"]["github_url"])
        self.assertIn("HEAD", payload["files"]["src/lib.rs"]["status"])

    def test_compiler_spans_must_refer_to_confined_existing_source_lines(self):
        for path, line in (("../outside.rs", 1), ("src/lib.rs", 100), ("src/lib.rs", True)):
            with self.subTest(path=path, line=line):
                self.api.update(path=path, line=line)
                with self.assertRaises(ValueError):
                    viewer.capture(self.root, self.bundle)

    def test_worktree_git_file_and_common_git_directory_are_protected(self):
        self.init_git()
        worktree = self.root / "worktree"
        self.git("worktree", "add", "-q", "-b", "fixture-worktree", str(worktree))
        for destination in (worktree / ".git", self.root / ".git/config"):
            with self.subTest(destination=destination):
                with self.assertRaisesRegex(ValueError, "refusing to overwrite"):
                    viewer.write_viewer(worktree, self.bundle, destination)

    def test_non_github_origin_has_no_fabricated_link(self):
        self.init_git()
        self.git("remote", "set-url", "origin", "https://example.invalid/example/sample.git")
        payload = viewer.capture(self.root, self.bundle)
        self.assertIsNone(payload["files"]["src/lib.rs"]["github_url"])

    def test_generated_source_is_portable_and_has_no_github_link(self):
        path = "@generated/sample/wire.rs"
        self.bundle["generated_sources"] = {path: "pub fn generated() {}\n"}
        self.api["path"] = path
        payload = viewer.capture(self.root, self.bundle)
        self.assertEqual(payload["files"][path]["content"], "pub fn generated() {}\n")
        self.assertIsNone(payload["files"][path]["github_url"])
        self.assertIn("Generated Rust source", payload["files"][path]["status"])
        with self.assertRaisesRegex(ValueError, "refusing to overwrite"):
            viewer.write_viewer(self.root, self.bundle, self.root / "@generated/sample/output.html")


if __name__ == "__main__":
    unittest.main()
