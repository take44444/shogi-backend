#!/usr/bin/env python3
import contextlib
import copy
import importlib.util
import io
import json
from pathlib import Path
import shlex
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

CHECKER = Path(__file__).with_name("traceability.py").resolve()
spec = importlib.util.spec_from_file_location("traceability", CHECKER)
traceability = importlib.util.module_from_spec(spec)
spec.loader.exec_module(traceability)


def source(package, needle):
    return {"path": f"crates/{package}/src/lib.rs", "needle": needle}


def contract(owner, local, consumer, provider, interface="RustApi"):
    return {"id": f"{owner}/{local}", "owner": owner, "record": f"crates/{owner}/src/trace/contracts/{local}.rs",
            "consumer": consumer, "provider": provider, "interface": interface, "statement": "境界の応答を定義する。",
            "assumptions": [], "limitations": [], "source": source(owner, "// interface"),
            "call": source(consumer, "// call") if consumer != "frontend" else None,
            "implementation": None, "binding": None, "requires": [], "gap": None}


def evidence(ref, provider, **changes):
    return dict({"contract": ref, "relationship": "asserts", "scope": "実提供元の境界を検査する。", "reason": "応答を比較する。",
                 "observation": "ProviderBoundary", "real": [provider], "fake": []}, **changes)


class TraceabilityTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="trace-validator-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        packages = [{"name": name, "path": f"crates/{name}"} for name in ("sample", "reader", "store")]
        components = []
        for package in packages:
            name = package["name"]
            base = self.root / package["path"]
            (base / "src/trace/contracts").mkdir(parents=True)
            (base / "src/trace/features").mkdir()
            (base / "src/trace/component.rs").touch()
            (base / "src/lib.rs").write_text("// interface\n// call\n// feature\n// implementation\n// binding\nfn internal() {}\nfn infrastructure() {}\nfn helper() {}\n")
            components.append({"id": name, "package": name, "path": package["path"], "description": "所属コンポーネント。", "notes": [],
                               "external": [], "operations": [], "entrypoints": [], "record": f"{package['path']}/src/trace/component.rs"})
        components[0]["external"] = [{"id": "frontend", "title": "ブラウザー。"}, {"id": "postgres", "title": "永続ストア。"}]
        api = contract("sample", "api", "frontend", "sample", "Http")
        lookup = contract("reader", "lookup", "sample", "reader")
        grpc = contract("reader", "grpc", "sample", "reader", "Grpc")
        grpc["gap"] = "内部 RPC の直接の検証は未収録。"
        api["requires"] = [{"contract": lookup["id"], "reason": "読み取り結果から応答を作る。"}]
        components[0]["operations"] = [{"contract": api["id"], "audience": "Frontend", "reason": "", "surface": {"kind": "OpenApi", "package": "sample", "example": "schema", "operation": "GET /ratings"}}]
        components[1]["operations"] = [{"contract": grpc["id"], "audience": "Internal", "reason": "サーバー間で利用する。", "surface": {"kind": "Grpc", "path": "proto/game.proto", "service": "GameService", "operation": "Selected"}}]
        feature = {"id": "ratings", "title": "レートの取得。", "source": source("sample", "// feature"), "flow": ["読み取り結果を返す。"],
                   "guarantees": [{"id": "public", "statement": "公開レートを返す。", "roots": [api["id"]]}], "limitations": [],
                   "owner": "sample", "record": "crates/sample/src/trace/features/ratings.rs"}
        boundary_path = "crates/sample/tests/contracts/api/tests.rs"
        boundary = {"id": "boundary", "package": "sample", "target": {"kind": "test", "name": "contracts"}, "name": "module::selected",
                    "source": {"path": boundary_path, "needle": "fn selected() {}", "line": 1}, "classification": "boundary",
                    "semantics": {"evidence": [evidence(api["id"], "sample", fake=["reader"]), evidence(lookup["id"], "reader")]}}
        (self.root / boundary_path).parent.mkdir(parents=True)
        (self.root / boundary_path).write_text("fn selected() {}\n")
        tests = [boundary]
        for name, line, semantics in (("internal", 6, {}), ("infrastructure", 7, {"category": "dependency-policy", "reason": "依存方針を確認する。"}), ("helper", 8, {"reason": "親テストから実行する。"})):
            tests.append({"id": name, "package": "sample", "target": {"kind": "lib"}, "name": f"module::{name}",
                          "source": dict(source("sample", f"fn {name}() {{}}"), line=line), "classification": name, "semantics": semantics})
        self.bundle = {"version": 2, "components": components, "features": [feature], "contracts": [api, lookup, grpc], "tests": tests,
                       "inventories": [{"package": "sample", "target": boundary["target"], "tests": [boundary["name"]]},
                                       {"package": "sample", "target": {"kind": "lib"}, "tests": [test["name"] for test in tests[1:]]}],
                       "packages": packages, "examples": [{"package": "sample", "example": "schema", "source": "crates/sample/src/lib.rs", "kind": "OpenApi", "reason": "公開仕様を出力する。"}], "excluded_targets": []}
        for node in [feature, api, lookup, grpc]:
            (self.root / node["record"]).touch()
        (self.root / "proto").mkdir()
        (self.root / "proto/game.proto").write_text("service GameService { rpc Selected(Request) returns (Response); }\n")
        self.protocol = {"inventory": {"sample|test|contracts": "module::selected: test\n", "sample|lib|": "module::internal: test\nmodule::infrastructure: test\nmodule::helper: test\n"}}
        self.commands = []

    def load_fixture(self, root):
        return traceability.validate_bundle(root, self.bundle)

    def execute_fixture(self, args, root):
        self.commands.append(args)
        if args[0:2] == ["cargo", "metadata"]:
            return json.dumps(self.protocol.get("metadata", {"packages": []}))
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
                code = traceability.main(shlex.split(command) + ["--root", str(self.root)])
        return SimpleNamespace(returncode=code, stdout=stdout.getvalue(), stderr=stderr.getvalue())

    def succeeds(self, command="check"):
        result = self.run_cli(command)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result

    def rejects(self, message, command="check"):
        result = self.run_cli(command)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn(message, result.stderr)
        self.assertNotIn("Traceback", result.stderr)
        return result

    def add_contract(self, owner, local, consumer, provider, interface="RustApi"):
        node = contract(owner, local, consumer, provider, interface)
        node["gap"] = "直接の検証は未収録。"
        (self.root / node["record"]).touch()
        self.bundle["contracts"].append(node)
        return node

    def test_check_is_read_only_and_run_selects_all_nonhelpers(self):
        self.succeeds()
        self.assertFalse(any(command[:2] == ["cargo", "test"] and "--list" not in command for command in self.commands))
        self.assertFalse((self.root / "docs").exists())
        self.assertFalse((self.root / "target").exists())
        self.commands.clear()
        output = self.succeeds("run").stdout
        self.assertIn("PASS boundary", output)
        self.assertIn("PASS internal", output)
        self.assertIn("PASS infrastructure", output)
        self.assertNotIn("PASS helper", output)
        self.assertEqual(sum("--list" in command for command in self.commands), 2)
        self.assertEqual(sum(command[:2] == ["cargo", "test"] and "--list" not in command for command in self.commands), 3)

    def test_run_kind_filters_keep_exhaustive_inventory(self):
        for kind in ("boundary", "internal", "infrastructure", "all"):
            with self.subTest(kind=kind):
                self.commands.clear()
                output = self.succeeds(f"run --kind {kind}").stdout
                self.assertEqual(output.count("PASS "), 3 if kind == "all" else 1)
                self.assertEqual(sum("--list" in command for command in self.commands), 2)
        self.protocol["inventory"]["sample|lib|"] += "missing: test\n"
        self.rejects("global classification inventory mismatch", "run --kind boundary")

    def test_shared_contracts_and_tests_are_single_global_entities(self):
        feature = copy.deepcopy(self.bundle["features"][0])
        feature.update(id="other", record="crates/sample/src/trace/features/other.rs")
        (self.root / feature["record"]).touch()
        self.bundle["features"].append(feature)
        self.succeeds()
        self.assertEqual(self.succeeds("run --kind boundary").stdout.count("PASS "), 1)
        self.assertEqual(len(self.bundle["contracts"]), 3)

    def test_shared_lower_contract_can_have_multiple_frontend_parents(self):
        second = self.add_contract("sample", "second", "frontend", "sample", "Http")
        second["requires"] = copy.deepcopy(self.bundle["contracts"][0]["requires"])
        self.bundle["features"][0]["guarantees"][0]["roots"].append(second["id"])
        operation = copy.deepcopy(self.bundle["components"][0]["operations"][0])
        operation["contract"] = second["id"]
        operation["surface"]["operation"] = "GET /other"
        self.bundle["components"][0]["operations"].append(operation)
        self.protocol["openapi"] = {"paths": {"/ratings": {"get": {}}, "/other": {"get": {}}}}
        self.succeeds()
        self.assertEqual(self.succeeds("run --kind boundary").stdout.count("PASS "), 1)
        second["gap"] = None
        self.rejects("asserts evidence or explicit gap required")

    def test_detached_internal_contract_needs_gap_or_direct_asserts(self):
        detached = self.add_contract("store", "database", "store", "postgres", "External")
        self.succeeds()
        detached["gap"] = None
        self.rejects("asserts evidence or explicit gap required")
        self.bundle["tests"][0]["semantics"]["evidence"].append(evidence(detached["id"], "postgres"))
        self.succeeds()

    def test_port_ownership_binding_and_production_dependency(self):
        port = self.add_contract("reader", "port", "sample", "store", "Port")
        port.update(implementation=source("store", "// implementation"), binding=source("sample", "// binding"))
        self.bundle["contracts"][0]["requires"].append({"contract": port["id"], "reason": "永続ストアを利用する。"})
        self.succeeds()
        (self.root / "Cargo.toml").touch()
        self.protocol["metadata"] = {"packages": [{"name": "store", "dependencies": [{"name": "reader", "kind": None}]}]}
        self.succeeds()
        self.protocol["metadata"]["packages"][0]["dependencies"][0]["kind"] = "dev"
        self.rejects("production dependency on interface owner")
        self.protocol["metadata"]["packages"][0]["dependencies"][0]["kind"] = None
        port["implementation"] = None
        self.rejects("Port implementation and binding required")
        port["implementation"] = source("reader", "// implementation")
        self.rejects("source ownership mismatch")

    def test_requires_reject_unknown_self_duplicate_nonadjacent_and_cycles(self):
        api = self.bundle["contracts"][0]
        edge = api["requires"][0]
        original = edge["contract"]
        edge["contract"] = "reader/missing"
        self.rejects("unknown required contract")
        edge["contract"] = api["id"]
        self.rejects("self reference")
        edge["contract"] = original
        api["requires"].append(copy.deepcopy(edge))
        self.rejects("duplicate association")
        api["requires"].pop()
        child = self.add_contract("store", "other", "reader", "store")
        edge["contract"] = child["id"]
        self.rejects("nonadjacent required contract")
        edge["contract"] = original
        loop = self.add_contract("sample", "loop", "reader", "sample")
        self.bundle["contracts"][1]["requires"] = [{"contract": loop["id"], "reason": "循環の検査。"}]
        loop["requires"] = [{"contract": original, "reason": "循環の検査。"}]
        self.rejects("contract cycle")

    def test_feature_roots_reject_internal_unknown_empty_and_missing_operations(self):
        refs = self.bundle["features"][0]["guarantees"][0]["roots"]
        refs[0] = "reader/lookup"
        self.rejects("feature root must be frontend boundary")
        refs[0] = "reader/missing"
        self.rejects("unknown root contract")
        refs.clear()
        self.rejects("nonempty list")
        refs.append("sample/api")
        self.bundle["components"][0]["operations"].clear()
        self.rejects("feature root lacks declared operation")

    def test_frontend_operations_require_roots_and_internal_operations_require_reasons(self):
        self.bundle["features"].clear()
        self.rejects("frontend operation must be feature root")
        self.bundle["components"][0]["operations"].clear()
        self.bundle["examples"].clear()
        self.succeeds()
        self.bundle["components"][1]["operations"][0]["reason"] = ""
        self.rejects("single-line text")

    def test_frontend_grpc_requires_explicit_audience(self):
        api = self.bundle["contracts"][0]
        api["interface"] = "Grpc"
        self.bundle["examples"].clear()
        self.bundle["components"][0]["operations"][0]["surface"] = {"kind": "Grpc", "path": "proto/game.proto", "service": "GameService", "operation": "Other"}
        (self.root / "proto/game.proto").write_text("service GameService { rpc Selected(Request) returns (Response); rpc Other(Request) returns (Response); }")
        self.succeeds()
        self.bundle["components"][0]["operations"][0]["audience"] = "Internal"
        self.bundle["components"][0]["operations"][0]["reason"] = "内部操作として分類する。"
        self.rejects("internal/operator operation cannot be feature root")

    def test_websocket_operation_requires_unique_real_anchor(self):
        self.bundle["examples"].clear()
        self.bundle["contracts"][0]["interface"] = "WebSocket"
        self.bundle["components"][0]["operations"][0]["surface"] = {"kind": "WebSocket", "path": "/ws", "command": "move", "source": source("sample", "// interface")}
        self.succeeds()
        self.bundle["components"][0]["operations"][0]["surface"]["source"] = source("reader", "// interface")
        self.rejects("source ownership mismatch")

    def test_evidence_fake_provider_and_complete_consumer_are_rejected(self):
        edge = self.bundle["tests"][0]["semantics"]["evidence"][0]
        edge["fake"] = ["sample"]
        self.rejects("real and fake participants overlap")
        edge["real"] = ["reader"]
        self.rejects("evidence provider must be real")
        edge["fake"] = []
        edge["real"] = ["sample"]
        edge["observation"] = "CompleteInteraction"
        self.rejects("complete interaction consumer must be real")
        edge["real"].append("frontend")
        self.succeeds()

    def test_exercises_and_unreviewed_never_replace_asserts_or_gap(self):
        edge = self.bundle["tests"][0]["semantics"]["evidence"][0]
        for relationship in ("exercises", "unreviewed"):
            with self.subTest(relationship=relationship):
                edge["relationship"] = relationship
                self.rejects("asserts evidence or explicit gap required")
                self.bundle["contracts"][0]["gap"] = "この境界を通過するだけ。"
                self.succeeds()
                self.bundle["contracts"][0]["gap"] = None

    def test_internal_infrastructure_and_helpers_cannot_claim_evidence(self):
        for test in self.bundle["tests"][1:]:
            with self.subTest(kind=test["classification"]):
                test["semantics"]["evidence"] = []
                self.rejects("expected fields")
                del test["semantics"]["evidence"]

    def test_boundary_placement_and_internal_placement_are_enforced(self):
        test = self.bundle["tests"][0]
        test["target"] = {"kind": "lib"}
        self.rejects("boundary tests must be integration")
        test["target"] = {"kind": "test", "name": "contracts"}
        test["classification"], test["semantics"] = "internal", {}
        self.rejects("internal test must be in src/")

    def test_contract_source_and_call_ownership_and_same_component(self):
        api, lookup = self.bundle["contracts"][:2]
        api["source"] = source("reader", "// interface")
        self.rejects("source ownership mismatch")
        api["source"] = source("sample", "// interface")
        lookup["call"] = None
        self.rejects("workspace consumer call anchor required")
        lookup["call"] = source("reader", "// call")
        self.rejects("source ownership mismatch")
        lookup["call"] = source("sample", "// call")
        lookup["consumer"] = "reader"
        self.rejects("same component contract")

    def test_duplicate_and_unknown_ids_and_bad_shapes(self):
        self.bundle["contracts"].append(copy.deepcopy(self.bundle["contracts"][0]))
        self.rejects("duplicate contract ID")
        self.bundle["contracts"].pop()
        self.bundle["tests"].append(copy.deepcopy(self.bundle["tests"][0]))
        self.rejects("global test IDs: duplicate")
        self.bundle["tests"].pop()
        self.bundle["contracts"][0]["provider"] = "unknown"
        self.rejects("unknown component")
        self.bundle["contracts"][0]["provider"] = "sample"
        self.bundle["extra"] = True
        self.rejects("expected fields")

    def test_sources_reject_stale_ambiguous_escape_absolute_and_symlink(self):
        anchor = self.bundle["features"][0]["source"]
        for path in ("missing.rs", "../source.rs", str(self.root / "crates/sample/src/lib.rs")):
            with self.subTest(path=path):
                anchor["path"] = path
                self.rejects("source")
        (self.root / "outside").symlink_to("/etc/passwd")
        anchor["path"] = "outside"
        self.rejects("outside repository")
        anchor["path"] = "crates/sample/src/lib.rs"
        for needle in ("missing needle", "//"):
            anchor["needle"] = needle
            self.rejects("exactly once")

    def test_parsed_test_source_line_and_function_are_checked(self):
        test = self.bundle["tests"][0]
        test["source"]["line"] = 0
        self.rejects("invalid parsed Rust source line")
        test["source"]["line"] = 1
        test["source"]["needle"] = "fn changed() {}"
        self.rejects("source needle must name")

    def test_global_inventory_rejects_new_hidden_missing_and_duplicate_compiled(self):
        self.protocol["inventory"]["sample|test|contracts"] = "module::selected_extra: test\n"
        self.rejects("exactly one compiled test")
        self.protocol["inventory"]["sample|test|contracts"] = "module::selected: test\n" * 2
        self.rejects("exactly one compiled test")
        self.protocol["inventory"]["sample|test|contracts"] = "module::selected: test\nhidden: test\n"
        self.rejects("unclassified compiled ['hidden']")
        self.protocol["inventory"]["sample|test|contracts"] = "module::selected: test\n"
        self.bundle["inventories"].append({"package": "reader", "target": {"kind": "bin", "name": "worker"}, "tests": []})
        self.protocol["inventory"]["reader|bin|worker"] = "hidden: test\n"
        self.rejects("global classification inventory mismatch")

    def test_inventory_includes_helpers_and_zero_test_targets(self):
        self.bundle["inventories"].append({"package": "reader", "target": {"kind": "lib"}, "tests": []})
        self.succeeds()
        self.assertTrue(any("reader" in command and "--list" in command for command in self.commands))
        self.protocol["inventory"]["reader|lib|"] = "hidden: test\nhidden: test\n"
        self.rejects("unclassified compiled ['hidden', 'hidden']")

    def test_inventory_declarations_must_match_all_classified_tests(self):
        self.bundle["inventories"][1]["tests"].pop()
        self.rejects("parsed tests and inventory declarations differ")

    def test_command_failure_and_unsupported_listing(self):
        self.protocol["inventory_exit"] = True
        self.rejects("command failed")
        self.protocol["inventory_exit"] = False
        self.protocol["inventory"]["sample|test|contracts"] = "unrecognized output\n"
        self.rejects("unsupported Cargo inventory")

    def test_surface_inventory_is_independent_of_feature_roots(self):
        self.protocol["openapi"] = {"paths": {"/ratings": {"get": {}, "post": {}}}}
        self.rejects("uncataloged ['POST /ratings']")
        self.protocol["openapi"] = {"paths": {}}
        self.rejects("absent ['GET /ratings']")
        self.protocol["openapi"] = {"paths": {"/ratings": {"get": {}}}}
        self.bundle["examples"].append({"package": "reader", "example": "other", "source": "crates/reader/src/lib.rs", "kind": "OpenApi", "reason": "別の仕様。"})
        self.rejects("OpenAPI exporter set mismatch")

    def test_examples_compile_once_and_only_exporters_execute(self):
        self.bundle["examples"].append({"package": "sample", "example": "demo", "source": "crates/sample/src/lib.rs", "kind": "Program", "reason": "起動環境が必要。"})
        self.succeeds()
        self.assertEqual([command for command in self.commands if command[:2] == ["cargo", "check"]], [["cargo", "check", "--locked", "-p", "sample", "--examples"]])
        self.assertEqual([command[-1] for command in self.commands if command[:2] == ["cargo", "run"]], ["schema"])
        self.protocol["example_check_exit"] = True
        self.rejects("example compilation failed")

    def test_grpc_exhaustive_inventory_and_unsupported_syntax(self):
        path = self.root / "proto/another.proto"
        path.write_text("service Another { rpc Added(Request) returns (Response); }")
        self.rejects("uncataloged [('proto/another.proto', 'Another', 'Added')]")
        path.unlink()
        path = self.root / "proto/game.proto"
        path.write_text("service GameService { rpc Selected(Request) returns (Response) { option x = true; } }")
        self.rejects("missing or unsupported gRPC service block")

    def test_unknown_legacy_command_and_output_argument_are_rejected(self):
        for arguments in (["render"], ["check", "--output", "out.html"], ["run", "--kind", "helper"]):
            with self.subTest(arguments=arguments), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as error:
                    traceability.main(arguments)
                self.assertEqual(error.exception.code, 2)

    def test_wrong_types_and_unknown_evidence_are_clean_validation_failures(self):
        cases = [
            ("version", True, "unsupported version"),
            ("features", {}, "nonempty list"),
            ("components", None, "nonempty list"),
        ]
        for field, value, error in cases:
            with self.subTest(field=field):
                previous = self.bundle[field]
                self.bundle[field] = value
                self.rejects(error)
                self.bundle[field] = previous
        edge = self.bundle["tests"][0]["semantics"]["evidence"][0]
        for field, value, error in (("contract", "sample/missing", "unknown evidence contract"), ("relationship", "proves", "invalid relationship"), ("observation", "Indirect", "invalid observation"), ("real", ["unknown"], "unknown evidence participant"), ("scope", "", "single-line text")):
            with self.subTest(field=field):
                previous = edge[field]
                edge[field] = value
                self.rejects(error)
                edge[field] = previous

    def test_default_components_and_internal_only_bundle_are_supported(self):
        self.bundle["features"].clear()
        self.bundle["contracts"].clear()
        self.bundle["tests"].pop(0)
        self.bundle["inventories"].pop(0)
        self.bundle["examples"].clear()
        (self.root / "proto/game.proto").unlink()
        for component in self.bundle["components"]:
            component.update(description=component["package"], notes=[], external=[], operations=[], entrypoints=[], record=None)
        self.succeeds()
        self.assertIn("PASS internal", self.succeeds("run").stdout)

    def test_duplicate_json_fields_are_rejected(self):
        with self.assertRaisesRegex(traceability.InvalidTrace, "duplicate JSON field"):
            json.loads('{"version": 2, "version": 1}', object_pairs_hook=traceability.no_duplicate_keys)

    def test_run_rejects_zero_ignored_multiple_failed_wrong_and_nonstandard_outcomes(self):
        self.succeeds("check")
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
                self.rejects("expected one passed exact test", "run --kind boundary")
        self.protocol["execution_exit"] = 1
        self.rejects("command failed", "run --kind boundary")

    def test_run_accepts_parent_success_with_nested_libtest_processes(self):
        name = "publisher_session_lock_excludes_a_second_process_and_releases_on_drop"
        self.bundle["tests"][0].update(name=name, source={"path": "crates/sample/tests/contracts/api/tests.rs", "needle": f"fn {name}() {{}}", "line": 1})
        (self.root / "crates/sample/tests/contracts/api/tests.rs").write_text(f"fn {name}() {{}}\n")
        self.bundle["inventories"][0]["tests"] = [name]
        self.protocol["inventory"]["sample|test|contracts"] = f"{name}: test\n"
        self.succeeds("check")
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
        self.assertIn("PASS boundary", self.succeeds("run --kind boundary").stdout)

    def test_run_uses_parent_result_when_nested_tests_are_empty_or_ignored(self):
        self.succeeds("check")
        children = [
            "running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s\n",
            "running 1 test\ntest child ... ignored\ntest result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 3 filtered out; finished in 0.00s\n",
        ]
        for child in children:
            with self.subTest(child=child):
                self.protocol["execution"] = ("running 1 test\n" + child + "test module::selected ... ok\n"
                                              "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n")
                self.succeeds("run --kind boundary")

    def test_run_accepts_serial_parent_completion_after_nested_child(self):
        self.succeeds("check")
        self.protocol["execution"] = (
            "running 1 test\ntest module::selected ... \n"
            "running 1 test\ntest child ... ok\n"
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n"
            "ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n")
        self.assertIn("PASS boundary", self.succeeds("run --kind boundary").stdout)

    def test_run_rejects_invalid_serial_parent_and_orphan_completions(self):
        self.succeeds("check")
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
                self.rejects("expected one passed exact test", "run --kind boundary")

    def test_run_rejects_unbalanced_malformed_and_repeated_runs(self):
        self.succeeds("check")
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
                self.rejects("expected one passed exact test", "run --kind boundary")


if __name__ == "__main__":
    unittest.main()
