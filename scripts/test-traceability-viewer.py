#!/usr/bin/env python3
import copy
import shutil
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import traceability_viewer as viewer



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
        anchor = lambda needle: {"path": self.source.name, "needle": needle}
        self.bundle = {
            "version": 2,
            "components": [{"id": "sample", "package": "sample", "path": ".", "description": "コンポーネント",
                            "notes": ["内部の注記"], "external": [{"id": "frontend", "title": "ブラウザー"}],
                            "operations": [], "entrypoints": [{"name": "起動", "source": anchor("// feature")}], "record": self.record.as_posix()}],
            "features": [{"id": "demo", "title": "日本語 </script><script>alert(1)</script>", "flow": ["処理"],
                          "owner": "sample", "record": self.record.as_posix(), "source": anchor("// feature"),
                          "guarantees": [{"id": "guarantee", "statement": "保証", "roots": ["sample/root"]},
                                         {"id": "shared", "statement": "共有の保証", "roots": ["sample/root"]}], "limitations": ["限界"]}],
            "contracts": [], "tests": [], "inventories": [], "packages": [], "examples": [], "excluded_targets": [],
        }
        for component in ("middle", "storage"):
            self.bundle["components"].append({"id": component, "package": component, "path": component,
                "description": component, "notes": [], "external": [], "operations": [], "entrypoints": [], "record": None})
        for name, consumer, provider, dependencies in [
            ("root", "frontend", "sample", ["middle/left", "middle/right"]),
            ("left", "sample", "middle", ["storage/leaf"]),
            ("right", "sample", "middle", ["storage/leaf"]),
            ("leaf", "middle", "storage", []),
        ]:
            self.bundle["contracts"].append({"id": f"{provider}/{name}", "owner": provider, "record": self.record.as_posix(),
                "consumer": consumer, "provider": provider, "interface": "Http" if name == "root" else "RustApi",
                "statement": f"契約 {name}", "source": anchor("// contract"), "call": anchor("// feature"),
                "implementation": anchor("fn selected() {}"), "binding": None,
                "assumptions": ["前提"], "limitations": ["限界"], "gap": "未検証事項",
                "requires": [{"contract": child, "reason": "下位の責務が必要"} for child in dependencies]})
        for classification in ["boundary", "internal", "infrastructure", "helper"]:
            semantics = {"evidence": [{"contract": "storage/leaf", "relationship": "asserts", "scope": "範囲", "reason": "根拠",
                                       "observation": "ProviderBoundary", "real": ["storage"], "fake": ["middle"]}]} if classification == "boundary" else {}
            if classification in ("infrastructure", "helper"):
                semantics = {"reason": "分類の理由"}
                if classification == "infrastructure":
                    semantics["category"] = "環境"
            self.bundle["tests"].append({"id": classification, "package": "sample", "target": {"kind": "test", "name": "contracts"},
                                        "name": f"module::{classification}", "source": {**anchor("fn selected() {}"), "line": 3},
                                        "classification": classification, "semantics": semantics})
        self.destination = self.root / "target/viewer.html"

    def traces(self):
        return self.bundle

    def export(self, bundle=None):
        viewer.write_viewer(self.root, bundle if bundle is not None else self.bundle, self.destination)
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

    def test_bundle_payload_and_all_source_anchors_are_captured_without_duplication(self):
        content, payload = self.export()
        self.assertEqual(payload["version"], 2)
        self.assertEqual(payload["bundle"], self.bundle)
        self.assertEqual(set(payload["files"]), {self.record.as_posix(), self.source.name})
        self.assertEqual(payload["files"][self.source.name]["content"].encode(), self.source.read_bytes())
        self.assertEqual(payload["locations"][self.source.name], {"// feature": 1, "// contract": 2, "fn selected() {}": 3})
        self.assertNotIn(str(self.root), content)
        self.assertNotIn('</script><script>alert(1)</script>', content)
        self.assertNotIn('<img src=x', content)
        self.assertNotIn("/* VIEWER_", content)
        self.assertNotIn("<!-- VIEWER_DATA -->", content)
        first = self.destination.read_bytes()
        self.export()
        self.assertEqual(first, self.destination.read_bytes())

    def test_gaps_shared_contracts_and_all_classifications_are_preserved(self):
        self.bundle["tests"][0]["semantics"]["evidence"] = []
        _, payload = self.export()
        self.assertEqual(payload["bundle"], self.bundle)
        self.assertEqual(len(payload["bundle"]["contracts"]), 4)
        self.assertEqual([test["classification"] for test in payload["bundle"]["tests"]],
                         ["boundary", "internal", "infrastructure", "helper"])
        self.assertEqual(payload["bundle"]["contracts"][0]["gap"], "未検証事項")

    def test_optional_anchors_and_records_are_validated(self):
        extra = self.root / "optional.rs"
        extra.write_text("fn bind() {}\n")
        self.bundle["contracts"][0]["binding"] = {"path": extra.name, "needle": "fn bind() {}"}
        _, payload = self.export()
        self.assertIn(extra.name, payload["files"])
        extra.write_text("fn changed() {}\n")
        with self.assertRaisesRegex(ValueError, "anchor changed"):
            self.export()

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
        traces["features"][0]["record"] = str(self.root / self.record)
        with self.assertRaisesRegex(ValueError, "repository-relative"):
            viewer.write_viewer(self.root, traces, self.destination)

    def test_rejects_source_paths_that_escape_the_repository(self):
        for path in ("../source.rs", str(self.source), "./" + self.source.name):
            self.bundle["features"][0]["source"]["path"] = path
            with self.subTest(path=path), self.assertRaisesRegex(ValueError, "repository-relative"):
                self.export()

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
        traces = self.traces()
        metadata = worktree / ".git"
        for destination in (metadata, self.root / ".git/worktrees/linked-worktree/HEAD", self.root / ".git/config"):
            with self.assertRaisesRegex(ValueError, "refusing to overwrite"):
                viewer.write_viewer(worktree, traces, destination)

    def test_browser_navigation_dag_classifications_and_source_snapshot(self):
        node = shutil.which("node")
        candidates = [Path(__file__).resolve().parents[1] / "node_modules/playwright"]
        candidates.extend((Path.home() / ".npm/_npx").glob("*/node_modules/playwright"))
        playwright = next((path for path in candidates if path.is_dir()), None)
        if node is None or playwright is None:
            self.skipTest("Node and Playwright are required for the browser driver")
        detached = copy.deepcopy(self.bundle["contracts"][-1])
        detached.update(id="middle/detached", owner="middle", consumer="sample", provider="storage", requires=[])
        self.bundle["contracts"].append(detached)
        extra_evidence = copy.deepcopy(self.bundle["tests"][0]["semantics"]["evidence"][0])
        extra_evidence["contract"] = "middle/detached"
        self.bundle["tests"][0]["semantics"]["evidence"].append(extra_evidence)
        for relationship in ("exercises", "unreviewed"):
            test = copy.deepcopy(self.bundle["tests"][0])
            test["id"] = relationship
            test["name"] = f"module::{relationship}"
            test["semantics"]["evidence"][0]["relationship"] = relationship
            self.bundle["tests"].append(test)
        self.export()
        component_only = copy.deepcopy(self.bundle)
        component_only["features"] = []
        component_destination = self.destination.with_name("components.html")
        viewer.write_viewer(self.root, component_only, component_destination)
        driver = r"""
const assert = require('node:assert/strict');
const { chromium } = require(process.argv[1]);
(async () => {
  const browser = await chromium.launch({headless: true});
  const page = await browser.newPage({viewport: {width: 1440, height: 1000}});
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(process.argv[2]);
  assert.equal(await page.locator('.graph-node[data-kind="contract"]').count(), 4);
  assert.equal(await page.locator('.graph-node[data-node-id="storage/leaf"]').count(), 1);
  const depth = await page.locator('.graph-node[data-kind="contract"]').evaluateAll(nodes => new Set(nodes.map(node => node.style.left)).size);
  assert.equal(depth, 3);
  assert.equal(await page.locator('.graph-node[data-kind="test"]').count(), 0);
  await page.locator('.graph-node[data-node-id="storage/leaf"]').click();
  for (const parent of ['middle/left', 'middle/right']) {
    assert.equal(await page.locator(`#details [data-related-id="${parent}"]`).count(), 1);
  }
  for (const relationship of ['asserts', 'exercises', 'unreviewed']) {
    assert.equal(await page.locator(`.edge-button[data-relationship="${relationship}"]`).count(), 1);
  }
  const evidence = page.locator('.edge-button[data-relationship="asserts"]');
  await evidence.focus();
  await page.keyboard.press('Enter');
  assert.match(await page.locator('#details').innerText(), /harness/);
  assert.match(await page.locator('#details').innerText(), /範囲/);
  const hash = new URL(page.url()).hash;
  assert.ok(!hash.includes('undefined'));
  await page.reload();
  assert.match(await page.locator('#details').innerText(), /harness/);
  await page.locator('#details [data-related-id="boundary"]').click();
  const source = page.locator('#details [data-source-open]');
  await source.click();
  assert.equal(await page.locator('#source-dialog').evaluate(dialog => dialog.open), true);
  assert.match(await page.locator('#source-lines [aria-current="true"]').innerText(), /fn selected/);
  await page.keyboard.press('Escape');
  assert.equal(await source.evaluate(control => document.activeElement === control), true);
  await page.locator('#details [data-related-id="middle/detached"]').click();
  assert.match(page.url(), /component=middle/);
  assert.match(page.url(), /id=middle%2Fdetached/);
  assert.equal(await page.locator('#notice').evaluate(node => node.hidden), true);
  assert.match(await page.locator('#details').innerText(), /sample → storage/);
  await page.reload();
  assert.match(await page.locator('#details').innerText(), /sample → storage/);
  await page.locator('[data-entry-kind="component"][data-entry-id="sample"]').click();
  for (const classification of ['internal', 'infrastructure', 'helper']) {
    const control = page.locator(`.graph-node[data-node-id="${classification}"]`);
    await control.focus();
    await page.keyboard.press('Enter');
    assert.match(await page.locator('#details').innerText(), /契約の根拠リンクを付けません/);
    assert.equal(await page.locator(`.edge-button[data-to="test:${classification}"]`).count(), 0);
  }
  await page.goBack();
  assert.match(page.url(), /id=infrastructure/);
  await page.locator('[data-entry-kind="feature"]').click();
  await page.locator('.graph-node[data-node-id="sample/root"]').click();
  assert.equal(await page.locator('#details [data-parent-feature="demo"]').count(), 2);
  assert.equal(await page.locator('.edge-button[data-relationship="asserts"]').count(), 0);
  assert.match(await page.locator('#details').innerText(), /直接の根拠はありません/);
  await page.evaluate(() => { location.hash = 'feature=demo&kind=contract&id=missing'; });
  await page.waitForFunction(() => !document.getElementById('notice').hidden);
  await page.goto(process.argv[3]);
  assert.equal(await page.locator('[data-entry-kind="feature"]').count(), 0);
  assert.equal(await page.locator('[data-entry-kind="component"]').count(), 3);
  await page.locator('[data-entry-kind="component"][data-entry-id="storage"]').click();
  await page.locator('.graph-node[data-node-id="storage/leaf"]').click();
  assert.equal(await page.locator('.edge-button[data-relationship="asserts"]').count(), 1);
  assert.deepEqual(errors, []);
  await browser.close();
})().catch(error => { console.error(error); process.exit(1); });
"""
        result = subprocess.run([node, "-e", driver, str(playwright), self.destination.as_uri(), component_destination.as_uri()], capture_output=True, text=True, timeout=90)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

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
