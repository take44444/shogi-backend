#!/usr/bin/env python3
import contextlib
import copy
import importlib.util
import io
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

CHECKER = Path(__file__).with_name("traceability.py").resolve()
RECORD = Path("crates/sample/src/trace/ratings/feature.rs")
DOCUMENT = Path("docs/reference/traceability/ratings.md")
INDEX = Path("docs/reference/feature-traceability.md")
spec = importlib.util.spec_from_file_location("traceability", CHECKER)
traceability = importlib.util.module_from_spec(spec)
spec.loader.exec_module(traceability)
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
        self.temporary = tempfile.TemporaryDirectory(prefix="trace-validator-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.graph = copy.deepcopy(BASE)
        self.records = {RECORD: self.graph}
        (self.root / "source.rs").write_text("// feature\n// reader\n// http\nfn selected() {}\nfn other() {}\n")
        (self.root / "proto").mkdir()
        (self.root / "proto/game.proto").write_text("service GameService { rpc Selected(Request) returns (Response); }\n")
        self.bundle = {
            "graphs": [], "inventories": [], "classifications": [], "excluded_targets": [],
            "packages": [{"name": "sample", "path": "crates/sample"}],
            "examples": [{"package": "sample", "example": "sample-openapi", "source": "source.rs", "kind": "OpenApi", "reason": "Exports the schema"}],
            "surfaces": [
                {"kind": "OpenApi", "package": "sample", "example": "sample-openapi", "operation": "GET /ratings", "feature": "ratings"},
                {"kind": "Grpc", "path": "proto/game.proto", "service": "GameService", "operation": "Selected", "feature": "ratings"},
            ],
            "entrypoints": [{"feature": "ratings", "name": "Worker", "source": {"path": "source.rs", "needle": "// feature"}}],
        }
        self.protocol = {"inventory": {"sample|lib|": "module::selected: test\n"}}
        self.commands = []

    def load_fixture(self, root):
        traceability.require(self.records, "no feature declarations")
        traces = traceability.Traces(self.bundle)
        for record, graph in self.records.items():
            graph, lines = traceability.validate_graph(root, graph)
            traces.append((record, graph, lines))
        traceability.unique([graph["feature"]["id"] for _, graph, _ in traces], "global feature IDs")
        traces.sort(key=lambda trace: trace[1]["feature"]["id"])
        return traces

    def execute_fixture(self, args, root):
        self.commands.append(args)
        if args[0:2] == ["cargo", "check"]:
            traceability.require(not self.protocol.get("example_check_exit"), "example compilation failed")
            return ""
        if args[0:2] == ["cargo", "run"]:
            traceability.require(not self.protocol.get("openapi_exit"), "command failed")
            return json.dumps(self.protocol.get("openapi", {"paths": {"/ratings": {"get": {}}}}))
        if "--list" in args:
            traceability.require(not self.protocol.get("inventory_exit"), "command failed")
            kind = "lib" if "--lib" in args else "test" if "--test" in args else "bin"
            target = "" if kind == "lib" else args[args.index("--" + kind) + 1]
            key = "|".join((args[args.index("-p") + 1], kind, target))
            return self.protocol["inventory"].get(key, "")
        traceability.require(not self.protocol.get("execution_exit"), "command failed")
        name = args[args.index("--") - 1]
        return self.protocol.get("execution", f"running 1 test\ntest {name} ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n")

    def run_cli(self, command):
        stdout, stderr = io.StringIO(), io.StringIO()
        with patch.object(traceability, "load_traces", self.load_fixture), patch.object(traceability, "execute", self.execute_fixture):
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                code = traceability.main([command, "--root", str(self.root)])
        return SimpleNamespace(returncode=code, stdout=stdout.getvalue(), stderr=stderr.getvalue())

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

    def add_feature(self):
        other = copy.deepcopy(BASE)
        other["feature"]["id"] = "other-feature"
        record = Path("apps/sample/src/trace/other-feature/feature.rs")
        self.records[record] = other
        self.bundle["entrypoints"].append({"feature": "other-feature", "name": "Other worker", "source": {"path": "source.rs", "needle": "// feature"}})
        return record, other

    def test_render_check_and_run_use_exact_identity_and_are_repeatable(self):
        self.succeeds("render")
        first = (self.root / DOCUMENT).read_bytes()
        self.succeeds("render")
        self.assertEqual(first, (self.root / DOCUMENT).read_bytes())
        self.succeeds("check")
        self.assertIn("PASS ratings/one", self.succeeds("run").stdout)
        self.assertEqual(self.commands[-1], ["cargo", "test", "--locked", "-p", "sample", "--lib", "module::selected", "--", "--exact", "--format", "pretty", "--color", "never"])
        self.assertIn("../../../source.rs#L4", first.decode())

    def test_multiple_features_share_inventory_and_execute_each_identity_once(self):
        self.add_feature()
        self.succeeds("render")
        self.commands.clear()
        self.assertIn("PASS other-feature/one, ratings/one", self.succeeds("run").stdout)
        self.assertEqual(sum("--list" in command for command in self.commands), 1)
        self.assertEqual(sum(command[1] == "test" and "--list" not in command for command in self.commands), 1)

    def test_global_inventory_rejects_unclassified_tests_in_empty_and_new_targets(self):
        for target in ({"kind": "lib"}, {"kind": "test", "name": "new_target"}, {"kind": "bin", "name": "worker"}):
            with self.subTest(target=target):
                self.bundle["inventories"] = [{"package": "sample", "target": target, "tests": ["module::selected"] if target["kind"] == "lib" else []}]
                key = "|".join(("sample", target["kind"], target.get("name", "")))
                self.protocol["inventory"][key] = ("module::selected: test\n" if target["kind"] == "lib" else "") + "hidden: test\n"
                self.rejects("global classification inventory mismatch")
                self.protocol["inventory"][key] = "module::selected: test\n" if target["kind"] == "lib" else ""

    def test_global_inventory_keeps_multiplicity_and_zero_test_targets(self):
        self.bundle["inventories"] = [{"package": "other", "target": {"kind": "lib"}, "tests": []}]
        self.succeeds("render")
        self.assertTrue(any("other" in command and "--list" in command for command in self.commands))
        self.protocol["inventory"]["other|lib|"] = "hidden: test\nhidden: test\n"
        self.rejects("unclassified compiled ['hidden', 'hidden']")

    def test_duplicate_global_feature_and_missing_all_declarations_are_rejected(self):
        record, other = self.add_feature()
        other["feature"]["id"] = "ratings"
        self.rejects("global feature IDs: duplicate")
        self.records.clear()
        self.rejects("no feature declarations")

    def test_surface_unknown_owners_duplicates_and_missing_entrypoint_anchor(self):
        self.bundle["surfaces"][0]["feature"] = "absent"
        self.rejects("unknown feature owner absent")
        self.bundle["surfaces"][0]["feature"] = "ratings"
        self.bundle["surfaces"].append(copy.deepcopy(self.bundle["surfaces"][0]))
        self.rejects("duplicate OpenAPI operation owner")
        self.bundle["surfaces"].pop()
        self.bundle["entrypoints"][0]["source"]["needle"] = "deleted worker"
        self.rejects("source needle must occur exactly once")

    def test_new_openapi_exporters_and_changed_operations_are_rejected(self):
        self.protocol["openapi"] = {"paths": {"/ratings": {"get": {}, "post": {}}}}
        self.rejects("uncataloged ['POST /ratings']")
        self.protocol["openapi"] = {"paths": {}}
        self.rejects("absent ['GET /ratings']")
        self.bundle["examples"].append({"package": "other", "example": "schema", "source": "source.rs", "kind": "OpenApi", "reason": "Exports another schema"})
        self.rejects("OpenAPI exporter set mismatch")

    def test_example_packages_compile_once_and_only_openapi_examples_execute(self):
        self.bundle["examples"].append({"package": "sample", "example": "demo", "source": "source.rs", "kind": "Program", "reason": "Needs a running service"})
        self.succeeds("render")
        checks = [args for args in self.commands if args[:2] == ["cargo", "check"]]
        self.assertEqual(checks, [["cargo", "check", "--locked", "-p", "sample", "--examples"]])
        runs = [args for args in self.commands if args[:2] == ["cargo", "run"]]
        self.assertEqual([args[-1] for args in runs], ["sample-openapi"])
        self.protocol["example_check_exit"] = True
        self.rejects("example compilation failed")

    def test_grpc_new_file_service_rpc_missing_and_unsupported_syntax_are_rejected(self):
        path = self.root / "proto/another.proto"
        path.write_text("service Another { rpc Added(Request) returns (Response); }")
        self.rejects("uncataloged [('proto/another.proto', 'Another', 'Added')]")
        path.unlink()
        path = self.root / "proto/game.proto"
        path.write_text("service GameService { rpc Added(Request) returns (Response); }")
        self.rejects("operation set mismatch")
        path.write_text("service GameService { rpc Selected(Request) returns (Response) { option x = true; } }")
        self.rejects("missing or unsupported gRPC service block")

    def test_generated_document_set_detects_missing_extra_then_render_repairs(self):
        self.succeeds("render")
        extra = self.root / "docs/reference/traceability/obsolete.md"
        extra.write_text("obsolete")
        self.rejects("generated document set mismatch", "check")
        self.succeeds("render")
        self.assertFalse(extra.exists())
        (self.root / DOCUMENT).unlink()
        self.rejects("generated document set mismatch", "check")
        self.succeeds("render")

    def test_rendered_table_columns_match_their_header(self):
        self.graph["feature"]["title"] = "Ratings | quoted `value`"
        self.succeeds("render")
        tables = 0
        for document in (DOCUMENT, INDEX):
            expected = None
            for line in (self.root / document).read_text().splitlines():
                if not line.startswith("|"):
                    expected = None
                    continue
                columns = len(line.split("|")) - 2
                if expected is None:
                    expected = columns
                    tables += 1
                self.assertEqual(columns, expected, line)
        self.assertGreater(tables, 0)

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
