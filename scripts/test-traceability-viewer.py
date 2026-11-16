#!/usr/bin/env python3
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import traceability_viewer as viewer

CHECKER = Path(__file__).with_name("traceability.py")
spec = importlib.util.spec_from_file_location("traceability", CHECKER)
traceability = importlib.util.module_from_spec(spec)
spec.loader.exec_module(traceability)


class ViewerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="trace-viewer-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "source 日本語.rs"
        self.source.write_bytes("// feature\r\n// contract\r\nfn selected() {}\r\n// </script><img src=x onerror=alert(1)> 日本語\r\n".encode())
        self.record = Path("trace/feature.rs")
        (self.root / self.record).parent.mkdir()
        (self.root / self.record).write_text("// feature declaration\n")
        self.graph = {
            "version": 1,
            "feature": {"id": "demo", "title": "日本語 </script><script>alert(1)</script>", "flow": ["処理"],
                        "source": {"path": self.source.name, "needle": "// feature"}},
            "guarantees": [{"id": "guarantee", "statement": "保証", "contracts": ["contract"]},
                           {"id": "shared", "statement": "共有の保証", "contracts": ["contract"]}],
            "contracts": [{"id": "contract", "statement": "契約", "source": {"path": self.source.name, "needle": "// contract"},
                           "gap": "未検証事項", "evidence": [{"test": "test", "relationship": "asserts", "reason": "根拠"}]}],
            "tests": [{"id": "test", "package": "sample", "target": {"kind": "lib"}, "name": "module::selected", "scope": "範囲",
                       "source": {"path": self.source.name, "needle": "fn selected() {}", "line": 3}}],
            "limitations": ["限界"],
        }
        self.destination = self.root / "target/viewer.html"

    def traces(self, graph=None):
        graph = graph or self.graph
        graph, lines = traceability.validate_graph(self.root, graph, parsed_tests=True)
        return [(self.record, graph, lines)]

    def export(self, traces=None):
        viewer.write_viewer(self.root, traces or self.traces(), self.destination)
        content = self.destination.read_text()
        start = '<script id="viewer-data" type="application/json">'
        data = content.split(start, 1)[1].split("</script>", 1)[0]
        return content, json.loads(data)

    def git(self, *arguments):
        result = subprocess.run(["git", "-C", str(self.root), *arguments], capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        return result.stdout.decode().strip()

    def repository(self, origin="git@github.com:example/project.git"):
        self.git("init", "--quiet")
        self.git("config", "user.name", "Viewer tests")
        self.git("config", "user.email", "viewer@example.test")
        self.git("add", ".")
        self.git("commit", "--quiet", "-m", "fixture")
        if origin:
            self.git("remote", "add", "origin", origin)
        return self.git("rev-parse", "HEAD")

    def test_graph_payload_is_preserved_scoped_and_sources_are_deduplicated(self):
        traces = self.traces()
        for relationship in ("exercises", "unreviewed"):
            other = copy.deepcopy(self.graph)
            other["feature"]["id"] = relationship
            other["contracts"][0]["evidence"][0]["relationship"] = relationship
            traces.extend(self.traces(other))
        content, payload = self.export(traces)
        self.assertEqual(payload["version"], 1)
        self.assertEqual(set(payload["files"]), {self.record.as_posix(), self.source.name})
        self.assertEqual(payload["files"][self.source.name]["content"].encode(), self.source.read_bytes())
        self.assertEqual(payload["features"][0]["graph"], self.graph)
        self.assertEqual(payload["features"][0]["locations"]["test"], {"path": self.source.name, "line": 3})
        self.assertNotIn("guarantee", payload["features"][0]["locations"])
        self.assertEqual([item["graph"]["contracts"][0]["evidence"][0]["relationship"] for item in payload["features"]],
                         ["asserts", "exercises", "unreviewed"])
        self.assertNotIn(str(self.root), content)
        self.assertNotIn('</script><script>alert(1)</script>', content)
        self.assertNotIn('<img src=x', content)
        self.assertNotIn("/* VIEWER_", content)
        self.assertNotIn("<!-- VIEWER_DATA -->", content)
        self.assertIsNone(payload["repository"]["revision"])
        self.assertIsNone(payload["files"][self.source.name]["github_url"])
        first = self.destination.read_bytes()
        self.export(traces)
        self.assertEqual(first, self.destination.read_bytes())

    def test_gap_without_tests_and_multiple_guarantees_are_preserved(self):
        self.graph["contracts"][0]["evidence"] = []
        self.graph["tests"] = []
        _, payload = self.export()
        graph = payload["features"][0]["graph"]
        self.assertEqual(graph["tests"], [])
        self.assertEqual(graph["contracts"][0]["gap"], "未検証事項")
        self.assertEqual(len(graph["guarantees"]), 2)

    def test_revision_links_require_exact_bytes_for_each_file(self):
        revision = self.repository()
        _, clean = self.export()
        expected = f"https://github.com/example/project/blob/{revision}/source%20%E6%97%A5%E6%9C%AC%E8%AA%9E.rs"
        self.assertEqual(clean["files"][self.source.name]["github_url"], expected)
        self.assertEqual(clean["repository"]["revision"], revision)
        self.source.write_bytes(self.source.read_bytes().replace(b"\r\n", b"\n"))
        _, dirty = self.export()
        self.assertIsNone(dirty["files"][self.source.name]["github_url"])
        self.assertIsNotNone(dirty["files"][self.record.as_posix()]["github_url"])
        self.git("add", self.source.name)
        _, staged = self.export()
        self.assertIsNone(staged["files"][self.source.name]["github_url"])
        self.source.write_bytes(clean["files"][self.source.name]["content"].encode())
        _, index_differs = self.export()
        self.assertEqual(index_differs["files"][self.source.name]["github_url"], expected)
        self.git("rm", "--cached", self.record.as_posix())
        self.git("commit", "--quiet", "-m", "remove declaration")
        _, untracked = self.export()
        self.assertIsNone(untracked["files"][self.record.as_posix()]["github_url"])

    def test_absent_git_or_unsupported_origin_keeps_local_source(self):
        self.repository("https://gitlab.com/example/project.git")
        _, payload = self.export()
        self.assertIsNone(payload["repository"]["github_base"])
        self.assertIsNone(payload["files"][self.source.name]["github_url"])
        with patch.object(viewer.subprocess, "run", side_effect=FileNotFoundError):
            _, payload = self.export()
        self.assertIsNone(payload["repository"]["revision"])
        self.assertIn("fn selected", payload["files"][self.source.name]["content"])

    def test_supported_github_origin_forms_and_rejected_origins(self):
        for origin in ("https://github.com/example/project.git", "https://github.com/example/project", "git@github.com:example/project.git", "ssh://git@github.com/example/project.git"):
            self.assertEqual(viewer.github_base(origin.encode()), "https://github.com/example/project")
        for origin in ("https://github.com.evil/example/project", "https://github.com/example/project/tree/main", "http://github.com/example/project", "https://user@github.com/example/project", "https://github.com/../project"):
            self.assertIsNone(viewer.github_base(origin.encode()))

    def test_changed_or_invalid_utf8_source_preserves_old_output(self):
        self.export()
        before = self.destination.read_bytes()
        traces = self.traces()
        self.source.write_text("// inserted\n" + self.source.read_text())
        with self.assertRaisesRegex(ValueError, "anchor changed"):
            viewer.write_viewer(self.root, traces, self.destination)
        self.assertEqual(self.destination.read_bytes(), before)
        self.source.write_bytes(b"\xff")
        with self.assertRaises(UnicodeDecodeError):
            viewer.write_viewer(self.root, traces, self.destination)
        self.assertEqual(self.destination.read_bytes(), before)

    def test_parsed_test_line_is_revalidated_against_captured_source(self):
        traces = self.traces()
        self.source.write_text(self.source.read_text().replace("fn selected() {}", "fn other() {}"))
        with self.assertRaisesRegex(ValueError, "anchor changed"):
            viewer.write_viewer(self.root, traces, self.destination)
        self.assertFalse(self.destination.exists())

    def test_declaration_path_cannot_embed_absolute_checkout_paths(self):
        traces = self.traces()
        traces[0] = (self.root / self.record, traces[0][1], traces[0][2])
        with self.assertRaisesRegex(ValueError, "repository-relative"):
            viewer.write_viewer(self.root, traces, self.destination)

    def test_rejects_inputs_tracked_files_tool_assets_and_git_metadata(self):
        self.repository()
        unrelated = self.root / "unrelated.txt"
        unrelated.write_text("do not overwrite")
        self.git("add", unrelated.name)
        for destination in (self.source, self.root / self.record, unrelated, self.root / ".git/config", self.root / ".git/new-file",
                            Path(viewer.__file__), viewer.ASSETS / "viewer.css", viewer.ASSETS / "new.html"):
            before = destination.read_bytes() if destination.is_file() else None
            with self.subTest(destination=destination), self.assertRaisesRegex(ValueError, "refusing to overwrite"):
                viewer.write_viewer(self.root, self.traces(), destination)
            self.assertEqual(destination.read_bytes() if destination.is_file() else None, before)

    def test_rejects_symlink_to_input_and_actual_worktree_git_metadata(self):
        self.repository()
        link = self.root / "linked-output.html"
        link.symlink_to(self.source)
        with self.assertRaisesRegex(ValueError, "refusing to overwrite"):
            viewer.write_viewer(self.root, self.traces(), link)
        worktree = self.root / "linked-worktree"
        self.git("worktree", "add", "--detach", str(worktree), "HEAD")
        traces = [(record, graph, lines) for record, graph, lines in self.traces()]
        metadata = worktree / ".git"
        for destination in (metadata, self.root / ".git/worktrees/linked-worktree/HEAD", self.root / ".git/config"):
            with self.assertRaisesRegex(ValueError, "refusing to overwrite"):
                viewer.write_viewer(worktree, traces, destination)

    def test_replace_failure_preserves_output_and_cleans_temporary_file(self):
        self.export()
        before = self.destination.read_bytes()
        with patch.object(viewer.os, "replace", side_effect=OSError("fixture replacement failure")):
            with self.assertRaisesRegex(OSError, "replacement failure"):
                self.export()
        self.assertEqual(self.destination.read_bytes(), before)
        self.assertEqual(list(self.destination.parent.glob(".traceability-*")), [])


if __name__ == "__main__":
    unittest.main()
