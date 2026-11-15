#!/usr/bin/env python3
import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


CHECKER = Path(__file__).with_name("traceability.py").resolve()
RECORD = Path("crates/player-http/src/ratings.trace.json")
DOCUMENT = Path("docs/reference/traceability/ratings.md")
INDEX = Path("docs/reference/feature-traceability.md")
CATALOG = Path("scripts/traceability-surfaces.json")
BASE = {
    "version": 1,
    "feature": {"id": "ratings", "title": "Ratings", "flow": ["HTTP delegates to a reader"], "source": {"path": "source.rs", "needle": "// feature"}},
    "guarantees": [{"id": "public", "statement": "Both pools", "contracts": ["reader", "http"]}],
    "contracts": [
        {"id": name, "statement": f"{name} contract", "source": {"path": "source.rs", "needle": f"// {name}"},
         "evidence": [{"test": "one", "relationship": "asserts", "reason": "Compares the result"}]}
        for name in ("reader", "http")
    ],
    "tests": [{"id": "one", "package": "sample", "target": {"kind": "lib"}, "name": "module::selected",
               "source": {"path": "source.rs", "needle": "fn selected() {}"}, "scope": "Fake repository"}],
    "limitations": ["No end-to-end assertion"],
}


class TraceabilityTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="ratings-trace-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.graph = copy.deepcopy(BASE)
        self.records = {RECORD: self.graph}
        self.catalog = {
            "version": 1,
            "openapi": [{"package": "sample", "example": "sample-openapi", "operations": {"GET /ratings": "ratings"}}],
            "grpc": {"path": "proto/game.proto", "service": "GameService", "operations": {"Selected": "ratings"}},
            "entrypoints": [{"feature": "ratings", "name": "Worker", "source": {"path": "source.rs", "needle": "// feature"}}],
        }
        (self.root / CATALOG).parent.mkdir(parents=True)
        (self.root / "proto").mkdir()
        (self.root / "proto/game.proto").write_text("service GameService {\n rpc Selected(Request) returns (Response);\n}\n")
        example = self.root / "crates/sample/examples/sample-openapi.rs"
        example.parent.mkdir(parents=True)
        example.write_text("fn main() {}\n")
        (example.parent.parent / "Cargo.toml").write_text('[package]\nname = "sample"\n')
        (self.root / RECORD).parent.mkdir(parents=True)
        (self.root / "source.rs").write_text("// feature\n// reader\n// http\nfn selected() {}\nfn other() {}\n")
        self.protocol = {"inventory": {"sample|lib|": "module::selected: test\n"}}
        self.bin = self.root / "bin"
        self.bin.mkdir()
        cargo = self.bin / "cargo"
        cargo.write_text(f"#!{sys.executable}\n" + '''
import json
from pathlib import Path
import sys
args = sys.argv[1:]
with Path("commands.jsonl").open("a") as log:
    log.write(json.dumps(args) + "\\n")
protocol = json.loads(Path("protocol.json").read_text())
package = args[args.index("-p") + 1]
if args[0] == "run":
    print(json.dumps(protocol.get("openapi", {"paths": {"/ratings": {"get": {}}}})))
    sys.exit(protocol.get("openapi_exit", 0))
kind = "lib" if "--lib" in args else "test"
target = "" if kind == "lib" else args[args.index("--test") + 1]
key = "|".join((package, kind, target))
if "--list" in args:
    print(protocol["inventory"].get(key, ""), end="")
    sys.exit(protocol.get("inventory_exit", 0))
name = args[args.index("--") - 1]
output = f"running 1 test\\ntest {name} ... ok\\n\\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\\n"
print(protocol.get("execution", output), end="")
sys.exit(protocol.get("execution_exit", 0))
''')
        cargo.chmod(0o755)

    def run_cli(self, command):
        for record, graph in self.records.items():
            (self.root / record).parent.mkdir(parents=True, exist_ok=True)
            (self.root / record).write_text(json.dumps(graph))
        (self.root / CATALOG).write_text(json.dumps(self.catalog))
        (self.root / "protocol.json").write_text(json.dumps(self.protocol))
        env = dict(os.environ, PATH=f"{self.bin}{os.pathsep}{os.environ['PATH']}")
        return subprocess.run([sys.executable, str(CHECKER), command, "--root", str(self.root)],
                              capture_output=True, text=True, env=env)

    def succeeds(self, command):
        result = self.run_cli(command)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result

    def rejects(self, message, command="render"):
        result = self.run_cli(command)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn(message, result.stderr)
        self.assertNotIn("Traceback", result.stderr)
        return result

    def test_render_check_and_run_use_exact_identity_and_are_repeatable(self):
        self.succeeds("render")
        first = (self.root / DOCUMENT).read_bytes()
        self.succeeds("render")
        self.assertEqual(first, (self.root / DOCUMENT).read_bytes())
        self.succeeds("check")
        self.assertIn("PASS ratings/one", self.succeeds("run").stdout)
        commands = [json.loads(line) for line in (self.root / "commands.jsonl").read_text().splitlines()]
        self.assertEqual(commands[-1], ["test", "--locked", "-p", "sample", "--lib", "module::selected",
                                        "--", "--exact", "--format", "pretty", "--color", "never"])
        rendered = first.decode()
        self.assertIn("../../../source.rs#L4", rendered)
        self.assertIn("asserts", rendered)
        self.assertIn("No end-to-end assertion", rendered)

    def test_rendered_table_columns_match_their_header(self):
        self.graph["feature"]["title"] = "Ratings | quoted `value`"
        self.succeeds("render")
        expected_columns = None
        tables = 0
        for document in (DOCUMENT, INDEX):
            rows = []
            for line in (self.root / document).read_text().splitlines() + [""]:
                if not line.startswith("|"):
                    if rows:
                        self.assertGreaterEqual(len(rows), 3)
                        self.assertTrue(all(cell.strip() == "---" for cell in rows[1].split("|")[1:-1]))
                    expected_columns, rows = None, []
                    continue
                columns = len(line.split("|")) - 2
                if expected_columns is None:
                    expected_columns = columns
                    tables += 1
                self.assertEqual(columns, expected_columns, line)
                rows.append(line)
        self.assertGreater(tables, 0)

    def add_feature(self):
        other = copy.deepcopy(BASE)
        other["feature"]["id"] = "other-feature"
        record = Path("apps/sample/other.trace.json")
        self.records[record] = other
        self.catalog["entrypoints"].append({"feature": "other-feature", "name": "Other worker",
                                            "source": {"path": "source.rs", "needle": "// feature"}})
        return record, other

    def test_multiple_features_share_inventory_and_execute_each_identity_once(self):
        self.add_feature()
        self.succeeds("render")
        (self.root / "commands.jsonl").write_text("")
        result = self.succeeds("run")
        self.assertIn("PASS other-feature/one, ratings/one", result.stdout)
        commands = [json.loads(line) for line in (self.root / "commands.jsonl").read_text().splitlines()]
        self.assertEqual(sum("--list" in command for command in commands), 1)
        self.assertEqual(sum(command[0] == "test" and "--list" not in command for command in commands), 1)
        self.assertIn("other-feature.md", (self.root / INDEX).read_text())

    def test_missing_record_and_duplicate_global_feature_id(self):
        record, other = self.add_feature()
        self.succeeds("render")
        other["feature"]["id"] = "ratings"
        self.rejects("global feature IDs: duplicate")
        self.records.pop(record)
        (self.root / record).unlink()
        self.rejects("unknown feature owner other-feature")

    def test_missing_all_records_is_rejected(self):
        self.succeeds("render")
        self.records.clear()
        (self.root / RECORD).unlink()
        self.rejects("no feature records")

    def test_orphan_feature_and_missing_entrypoint_anchor(self):
        self.add_feature()
        self.catalog["entrypoints"].pop()
        self.rejects("orphan feature without catalog surface")
        self.catalog["entrypoints"][0]["source"]["needle"] = "deleted worker"
        self.rejects("source needle must occur exactly once")

    def test_openapi_addition_removal_unowned_operation_and_exporter_removal(self):
        self.protocol["openapi"] = {"paths": {"/ratings": {"get": {}, "post": {}}}}
        self.rejects("uncataloged ['POST /ratings']")
        self.protocol["openapi"] = {"paths": {}}
        self.rejects("absent ['GET /ratings']")
        self.catalog["openapi"][0]["operations"]["GET /ratings"] = "missing"
        self.rejects("unknown feature owner missing")
        self.catalog["openapi"] = []
        self.rejects("OpenAPI exporter set mismatch")

    def test_grpc_missing_operation_owner_service_and_unfamiliar_syntax(self):
        grpc = self.catalog["grpc"]
        grpc["operations"] = {}
        self.rejects("gRPC operation set mismatch")
        grpc["operations"] = {"Selected": "missing"}
        self.rejects("unknown feature owner missing")
        grpc["operations"] = {"Selected": "ratings"}
        grpc["service"] = "Missing"
        self.rejects("source needle must occur exactly once")
        grpc["service"] = "GameService"
        (self.root / "proto/game.proto").write_text("service GameService { rpc Selected(Request) returns (Response) { option x = true; } }")
        self.rejects("missing or unsupported gRPC service block")

    def test_generated_document_set_detects_missing_extra_and_legacy_then_render_repairs(self):
        self.succeeds("render")
        extra = self.root / "docs/reference/traceability/obsolete.md"
        extra.write_text("obsolete")
        self.rejects("generated document set mismatch", "check")
        self.succeeds("render")
        self.assertFalse(extra.exists())
        (self.root / DOCUMENT).unlink()
        self.rejects("generated document set mismatch", "check")
        self.succeeds("render")
        legacy = self.root / "docs/reference/ratings-traceability.md"
        legacy.write_text("old pilot")
        self.rejects("generated document set mismatch", "check")
        self.succeeds("render")
        self.assertFalse(legacy.exists())

    def test_empty_tests_require_explicit_contract_gaps_and_flow_is_required(self):
        self.graph["tests"] = []
        for contract in self.graph["contracts"]:
            contract.update(evidence=[], gap="No direct assertion")
        self.succeeds("render")
        self.graph["feature"]["flow"] = []
        self.rejects("nonempty list")
        self.graph["feature"]["flow"] = ["two\nlines"]
        self.rejects("single-line text")

    def test_missing_required_guarantee_edge_and_orphan_contract(self):
        self.graph["guarantees"][0]["contracts"] = []
        self.rejects("nonempty list")
        self.graph["guarantees"][0]["contracts"] = ["reader"]
        self.rejects("orphan contract")

    def test_unknown_and_duplicate_associations(self):
        refs = self.graph["guarantees"][0]["contracts"]
        refs.append("missing")
        self.rejects("unknown contract")
        refs[-1] = "reader"
        self.rejects("duplicate association")

    def test_deleted_evidence_requires_explicit_gap(self):
        self.graph["contracts"][0]["evidence"] = []
        self.rejects("evidence or explicit gap required")
        self.graph["contracts"][0]["gap"] = "No direct assertion"
        self.succeeds("render")
        self.assertIn("未検証: No direct assertion", (self.root / DOCUMENT).read_text())

    def test_orphan_test_and_unknown_evidence_are_rejected(self):
        for contract in self.graph["contracts"]:
            contract["evidence"] = []
            contract["gap"] = "Missing"
        self.rejects("orphan test")
        self.graph["contracts"][0]["evidence"] = [{"test": "missing", "relationship": "asserts", "reason": "Missing"}]
        self.rejects("unknown test")

    def test_sources_reject_missing_escape_absolute_and_symlink_paths(self):
        for path in ("missing.rs", "../source.rs", str(self.root / "source.rs")):
            with self.subTest(path=path):
                self.graph["feature"]["source"]["path"] = path
                self.rejects("source")
        (self.root / "outside").symlink_to("/etc/passwd")
        self.graph["feature"]["source"]["path"] = "outside"
        self.rejects("outside repository")

    def test_stale_and_ambiguous_needles(self):
        self.graph["feature"]["source"]["needle"] = "not present"
        self.rejects("exactly once")
        self.graph["feature"]["source"]["needle"] = "//"
        self.rejects("exactly once")

    def test_source_needle_names_the_declared_test(self):
        self.graph["tests"][0]["source"]["needle"] = "fn other() {}"
        self.rejects("source needle must name")

    def test_stale_document_blocks_check_and_run(self):
        self.succeeds("render")
        (self.root / DOCUMENT).write_text("stale")
        self.rejects("stale generated document", "check")
        self.rejects("stale generated document", "run")
        self.succeeds("render")
        self.succeeds("check")

    def test_missing_exact_identity_is_not_a_substring_match(self):
        self.protocol["inventory"]["sample|lib|"] = "module::selected_extra: test\n"
        self.rejects("exactly one compiled test")

    def test_duplicate_compiled_and_declared_identities(self):
        self.protocol["inventory"]["sample|lib|"] *= 2
        self.rejects("exactly one compiled test")
        self.protocol["inventory"]["sample|lib|"] = "module::selected: test\n"
        other = dict(self.graph["tests"][0], id="two")
        self.graph["tests"].append(other)
        self.graph["contracts"][0]["evidence"].append({"test": "two", "relationship": "exercises", "reason": "Other"})
        self.rejects("Cargo tests: duplicate")

    def test_inventory_is_scoped_to_package_and_target_even_when_names_collide(self):
        other = copy.deepcopy(self.graph["tests"][0])
        other.update(id="two", package="other", target={"kind": "test", "name": "ratings"})
        self.graph["tests"].append(other)
        self.graph["contracts"][0]["evidence"].append({"test": "two", "relationship": "exercises", "reason": "Other"})
        self.rejects("two: expected exactly one")
        self.protocol["inventory"]["other|test|ratings"] = "module::selected: test\n"
        self.succeeds("render")
        self.succeeds("run")
        self.graph["tests"][1]["target"] = {"kind": "lib"}
        self.rejects("two: expected exactly one")

    def test_integration_target_names_accept_underscores(self):
        self.graph["tests"][0]["target"] = {"kind": "test", "name": "schema_contract"}
        self.protocol["inventory"]["sample|test|schema_contract"] = "module::selected: test\n"
        self.succeeds("render")
        self.succeeds("run")

    def test_wrong_package_cannot_use_another_packages_inventory(self):
        self.graph["tests"][0]["package"] = "other"
        self.rejects("expected exactly one")

    def test_command_failure_and_unsupported_listing_leave_document_unchanged(self):
        self.succeeds("render")
        previous = (self.root / DOCUMENT).read_bytes()
        self.protocol["inventory_exit"] = 1
        self.rejects("command failed")
        self.protocol["inventory_exit"] = 0
        self.protocol["inventory"]["sample|lib|"] = "unrecognized output\n"
        self.rejects("unsupported Cargo inventory")
        self.assertEqual(previous, (self.root / DOCUMENT).read_bytes())

    def test_run_rejects_zero_ignored_multiple_failed_wrong_and_nonstandard_outcomes(self):
        self.succeeds("render")
        cases = [
            "running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s\n",
            "running 1 test\ntest module::selected ... ignored\ntest result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 3 filtered out; finished in 0.00s\n",
            "running 2 tests\ntest module::selected ... ok\ntest module::other ... ok\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s\n",
            "running 1 test\ntest module::selected ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n",
            "running 1 test\ntest module::other ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n",
            "custom runner success\n",
        ]
        for output in cases:
            with self.subTest(output=output):
                self.protocol["execution"] = output
                self.rejects("expected one passed exact test", "run")
        self.protocol["execution_exit"] = 1
        self.rejects("command failed", "run")

    def test_run_accepts_parent_success_with_nested_libtest_processes(self):
        name = "publisher_session_lock_excludes_a_second_process_and_releases_on_drop"
        self.graph["tests"][0].update(name=name, source={"path": "source.rs", "needle": f"fn {name}() {{}}"})
        with (self.root / "source.rs").open("a") as source:
            source.write(f"fn {name}() {{}}\n")
        self.protocol["inventory"]["sample|lib|"] = f"{name}: test\n"
        self.succeeds("render")
        self.protocol["execution"] = """
running 1 test

running 1 test
test publisher_lock_probe ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.15s


running 1 test
test publisher_lock_probe ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.15s

test publisher_session_lock_excludes_a_second_process_and_releases_on_drop ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.88s
"""
        self.assertIn("PASS ratings/one", self.succeeds("run").stdout)

    def test_run_uses_parent_result_when_nested_tests_are_empty_or_ignored(self):
        self.succeeds("render")
        children = [
            "running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s\n",
            "running 1 test\ntest child ... ignored\ntest result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 3 filtered out; finished in 0.00s\n",
        ]
        for child in children:
            with self.subTest(child=child):
                self.protocol["execution"] = ("running 1 test\n" + child + "test module::selected ... ok\n"
                                              "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n")
                self.succeeds("run")

    def test_run_accepts_serial_parent_completion_after_nested_child(self):
        self.succeeds("render")
        self.protocol["execution"] = (
            "running 1 test\ntest module::selected ... \n"
            "running 1 test\ntest child ... ok\n"
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n"
            "ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n")
        self.assertIn("PASS ratings/one", self.succeeds("run").stdout)

    def test_run_rejects_invalid_serial_parent_and_orphan_completions(self):
        self.succeeds("render")
        child = ("running 1 test\ntest child ... ok\n"
                 "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n")
        parent = ("running 1 test\ntest module::selected ... \n" + child + "ok\n"
                  "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n")
        cases = [
            parent.replace("\nok\n", "\nignored\n"),
            parent.replace("\nok\n", "\nFAILED\n"),
            parent.replace("module::selected", "module::other"),
            parent.replace("\nok\n", "\n"),
            parent.replace("\nok\n", "\nunsupported\n"),
            parent.replace("test module::selected ... \n", "test module::selected ...\n"),
            parent.replace("\nok\n", "\ntest module::selected ... ok\n"),
            parent.replace("\nok\n", "\ntest module::selected ... \nok\n"),
            parent + "ok\n",
            parent + "FAILED\n",
            parent + "ignored\n",
            "ok\n" + parent,
            "running 1 test\nok\n" + parent,
            parent + parent,
        ]
        for output in cases:
            with self.subTest(output=output):
                self.protocol["execution"] = output
                self.rejects("expected one passed exact test", "run")

    def test_run_rejects_unbalanced_malformed_and_repeated_runs(self):
        self.succeeds("render")
        parent = ("running 1 test\ntest module::selected ... ok\n"
                  "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n")
        cases = [
            parent.removeprefix("running 1 test\n"),
            "running 1 test\n" + parent,
            parent + "running 0 tests\n",
            parent + "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n",
            parent + parent,
            parent.replace("running 1 test", "running unknown tests"),
            parent.replace("test result: ok.", "test result: unsupported."),
            "running 1 test\nrunning 0 tests\ntest result: unsupported\n" + parent.removeprefix("running 1 test\n"),
            parent.replace("running 1 test", "running 2 tests"),
            "running 1 test\n" + parent + "test module::selected ... ignored\n"
            "test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 3 filtered out; finished in 0.00s\n",
        ]
        for output in cases:
            with self.subTest(output=output):
                self.protocol["execution"] = output
                self.rejects("expected one passed exact test", "run")

    def test_unknown_fields_bad_relationships_duplicate_ids_and_wrong_types(self):
        self.graph["extra"] = "typo"
        self.rejects("expected fields")
        self.graph.pop("extra")
        self.graph["contracts"][0]["evidence"][0]["relationship"] = "proves"
        self.rejects("invalid relationship")
        self.graph["contracts"][0]["evidence"][0]["relationship"] = "asserts"
        self.graph["tests"][0]["id"] = "reader"
        self.rejects("graph IDs: duplicate")
        self.graph["tests"][0]["id"] = "one"
        self.graph["guarantees"][0]["contracts"] = "reader"
        self.rejects("nonempty list")


if __name__ == "__main__":
    unittest.main()
