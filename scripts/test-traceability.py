#!/usr/bin/env python3
from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("traceability", Path(__file__).with_name("traceability.py"))
trace = importlib.util.module_from_spec(spec)
spec.loader.exec_module(trace)


class CompilerReportTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        (self.root / "src").mkdir()
        (self.root / "src/lib.rs").write_text("pub fn api() {}\nfn caller() { api(); }\nfn check() {}\n")
        self.components = [{"id": "sample", "package": "sample", "path": "."}]
        self.api = {"id": "sample/api", "package": "sample", "name": "api", "kind": "function", "path": "src/lib.rs", "line": 1}
        self.caller = {**self.api, "id": "sample/caller", "name": "caller", "line": 2}
        self.shard = {
            "version": 3,
            "unit": {"package": "sample", "crate_name": "sample", "target_kind": "lib", "target_name": "sample", "is_test": True, "features": [], "purposes": ["tests"], "source": "src/lib.rs"},
            "apis": [self.api, self.caller],
            "calls": [{"caller": "sample/caller", "caller_package": "sample", "callee": "sample/api", "path": "src/lib.rs", "line": 2, "dispatch": "static"}],
            "tests": [{"package": "sample", "target_kind": "lib", "target_name": "sample", "name": "check", "path": "src/lib.rs", "line": 3, "targets": ["sample/api"]}],
            "unresolved": [], "coverage": {"bodies": 3, "calls": 1, "function_references": 0, "external_calls": 0},
        }

    def merge(self, *shards):
        return trace.merge_shards(self.root, shards or [self.shard], self.components)

    def test_merging_normal_and_test_units_deduplicates_calls_and_preserves_targets(self):
        normal = deepcopy(self.shard)
        normal["unit"]["is_test"] = False
        normal["tests"] = []
        first = self.merge(self.shard, normal, normal)
        second = self.merge(normal, self.shard)
        self.assertEqual(first, second)
        self.assertEqual(first["coverage"]["bodies"], 6)
        self.assertEqual(len(first["calls"]), 1)
        self.assertEqual(first["calls"][0]["origins"], trace.sorted_records([normal["unit"], self.shard["unit"]]))
        self.assertEqual(first["tests"][0]["targets"], ["sample/api"])
        self.assertEqual(len(first["units"]), 2)

    def test_conflicting_definition_is_rejected(self):
        normal = deepcopy(self.shard)
        normal["unit"]["is_test"] = False
        normal["tests"] = []
        normal["apis"][0]["line"] = 2
        with self.assertRaisesRegex(trace.InvalidTrace, "conflicting API definition"):
            self.merge(self.shard, normal)

    def test_every_call_and_test_reference_must_resolve(self):
        for category, field in (("calls", "callee"), ("calls", "caller"), ("tests", "targets")):
            with self.subTest(category=category, field=field):
                broken = deepcopy(self.shard)
                broken[category][0][field] = ["absent"] if field == "targets" else "absent"
                with self.assertRaisesRegex(trace.InvalidTrace, "unknown .*API"):
                    self.merge(broken)

    def test_test_target_does_not_promote_concrete_impl_to_interface(self):
        concrete = {**self.api, "id": "sample/concrete", "name": "<Concrete as Trait>::api", "kind": "trait-impl-method"}
        interface = {**self.api, "id": "sample/trait", "name": "Trait::api", "kind": "trait-method"}
        self.shard["apis"] += [concrete, interface]
        self.shard["calls"][0]["callee"] = interface["id"]
        self.shard["calls"][0]["dispatch"] = "dyn"
        self.shard["tests"][0]["targets"] = [concrete["id"]]
        bundle = self.merge()
        self.assertEqual(bundle["tests"][0]["targets"], ["sample/concrete"])
        self.assertEqual(bundle["calls"][0]["callee"], "sample/trait")

    def test_calls_do_not_assign_targets_to_unannotated_tests(self):
        self.shard["tests"][0]["targets"] = []
        bundle = self.merge()
        self.assertEqual(trace.selected_tests(bundle, "targeted"), [])
        self.assertEqual(trace.selected_tests(bundle, "untargeted"), bundle["tests"])
        self.assertEqual(trace.selected_tests(bundle, "all"), bundle["tests"])

    def test_missing_compiler_shards_and_wrong_unit_context_fail(self):
        with self.assertRaisesRegex(trace.InvalidTrace, "no compiler shards"):
            trace.merge_shards(self.root, [], self.components)
        self.shard["tests"][0]["target_name"] = "other"
        with self.assertRaisesRegex(trace.InvalidTrace, "incorrect compiler unit context"):
            self.merge()

    def test_source_locations_are_confined_and_lines_are_real(self):
        for path, line in (("../outside.rs", 1), ("/tmp/outside.rs", 1), ("src/lib.rs", 5), ("src/lib.rs", True)):
            with self.subTest(path=path, line=line):
                broken = deepcopy(self.shard)
                broken["apis"][0].update(path=path, line=line)
                with self.assertRaises(trace.InvalidTrace):
                    self.merge(broken)

    def test_symlink_source_outside_root_is_rejected(self):
        with tempfile.TemporaryDirectory() as outside:
            path = Path(outside) / "outside.rs"
            path.write_text("pub fn outside() {}\n")
            (self.root / "src/link.rs").symlink_to(path)
            self.shard["apis"][0]["path"] = "src/link.rs"
            with self.assertRaisesRegex(trace.InvalidTrace, "outside repository"):
                self.merge()

    def test_unresolved_calls_remain_visible(self):
        self.shard["unresolved"] = [{"caller": "sample/caller", "path": "src/lib.rs", "line": 2, "reason": "function pointer target unknown"}]
        self.assertEqual(self.merge()["unresolved"], [{**self.shard["unresolved"][0], "origins": [self.shard["unit"]]}])

    def test_call_reference_and_unresolved_origins_preserve_only_observed_units(self):
        self.shard["references"] = deepcopy(self.shard["calls"])
        self.shard["unresolved"] = [{"caller": "sample/caller", "path": "src/lib.rs", "line": 2, "reason": "function pointer target unknown"}]
        normal = deepcopy(self.shard)
        normal["unit"]["is_test"] = False
        normal["tests"] = []
        normal["calls"] = []
        for category in ("references", "unresolved"):
            normal[category] += deepcopy(normal[category])
        first = self.merge(self.shard, normal)
        self.assertEqual(first, self.merge(normal, self.shard, normal))
        self.assertEqual(first["calls"][0]["origins"], [self.shard["unit"]])
        for category in ("references", "unresolved"):
            self.assertEqual(len(first[category]), 1)
            self.assertEqual(first[category][0]["origins"], trace.sorted_records([normal["unit"], self.shard["unit"]]))

    def test_function_references_remain_separate_from_calls_and_validate_api_ids(self):
        self.shard["references"] = self.shard["calls"]
        self.shard["calls"] = []
        bundle = self.merge()
        self.assertEqual(bundle["calls"], [])
        self.assertEqual(len(bundle["references"]), 1)
        self.shard["references"][0]["callee"] = "absent"
        with self.assertRaisesRegex(trace.InvalidTrace, "unknown call callee API"):
            self.merge()

    def test_generated_sources_resolve_logical_spans_without_temporary_build_paths(self):
        path = "@generated/sample/wire.rs"
        self.shard["generated_sources"] = {path: "pub fn generated() {}\n"}
        self.shard["apis"].append({**self.api, "id": "sample/generated", "name": "generated", "path": path})
        bundle = self.merge()
        self.assertEqual(bundle["generated_sources"], {path: "pub fn generated() {}\n"})
        normal = deepcopy(self.shard)
        normal["unit"]["is_test"] = False
        normal["tests"] = []
        normal["generated_sources"][path] = "pub fn changed() {}\n"
        with self.assertRaisesRegex(trace.InvalidTrace, "conflicting generated source contents"):
            self.merge(self.shard, normal)

    def test_generated_sources_cannot_override_repository_files_or_escape_namespace(self):
        for key in ("src/lib.rs", "@generated/unknown/file.rs", "@generated/sample/../../file.rs"):
            with self.subTest(key=key):
                self.shard["generated_sources"] = {key: "fn bad() {}"}
                with self.assertRaisesRegex(trace.InvalidTrace, "invalid generated source key"):
                    self.merge()
        path = "@generated/sample/file.rs"
        (self.root / path).parent.mkdir(parents=True)
        (self.root / path).write_text("fn real() {}\n")
        self.shard["generated_sources"] = {path: "fn bad() {}"}
        with self.assertRaisesRegex(trace.InvalidTrace, "collides with repository file"):
            self.merge()

    def test_cargo_artifacts_reconcile_normal_and_test_units(self):
        metadata = {"workspace_members": ["sample-id"], "packages": [{"id": "sample-id", "name": "sample"}]}
        message = {"reason": "compiler-artifact", "package_id": "sample-id", "target": {"kind": ["lib"], "name": "sample"}, "profile": {"test": True}, "features": [], "executable": "/test-binary"}
        artifacts = trace.cargo_artifacts([("tests", json.dumps(message))], metadata)
        self.assertEqual(artifacts, trace.cargo_artifacts([("tests", json.dumps(message) + "\n" + json.dumps(message))], metadata))
        bundle = self.merge()
        trace.reconcile_units(bundle, artifacts)
        missing = {**artifacts[0], "is_test": False}
        with self.assertRaisesRegex(trace.InvalidTrace, "missing shards"):
            trace.reconcile_units(bundle, artifacts + [missing])

    def test_features_and_cargo_phases_preserve_distinct_normal_unit_origins(self):
        metadata = {"workspace_members": ["sample-id"], "packages": [{"id": "sample-id", "name": "sample"}]}
        message = {"reason": "compiler-artifact", "package_id": "sample-id", "target": {"kind": ["lib"], "name": "sample"},
                   "profile": {"test": False}, "features": [], "executable": None}
        feature_message = {**message, "features": ["extra"]}
        artifacts = trace.cargo_artifacts([("production", json.dumps(message)),
                                           ("tests", json.dumps(message) + "\n" + json.dumps(feature_message))], metadata)
        self.assertEqual({tuple(artifact["features"]): artifact["purposes"] for artifact in artifacts},
                         {(): ["production", "tests"], ("extra",): ["tests"]})
        normal = deepcopy(self.shard)
        normal["unit"]["is_test"] = False
        normal["unit"].pop("purposes")
        normal["tests"] = []
        feature = deepcopy(normal)
        feature["unit"]["features"] = ["extra"]
        normal["calls"] = []
        shards = trace.compilation_purposes([normal, feature], artifacts)
        bundle = self.merge(*shards)
        trace.reconcile_units(bundle, artifacts)
        self.assertEqual(len(bundle["units"]), 2)
        self.assertEqual(bundle["calls"][0]["origins"][0]["purposes"], ["tests"])
        self.assertFalse(bundle["calls"][0]["origins"][0]["is_test"])
        self.assertEqual(bundle, self.merge(*reversed(shards)))

    def test_features_and_purposes_are_required_canonical_unit_fields(self):
        for field, value in (("features", None), ("features", ["extra", "extra"]),
                             ("features", ["z", "a"]), ("features", [True]),
                             ("purposes", []), ("purposes", ["unknown"]),
                             ("purposes", ["tests", "tests"])):
            with self.subTest(field=field, value=value):
                shard = deepcopy(self.shard)
                shard["unit"][field] = value
                with self.assertRaises(trace.InvalidTrace):
                    self.merge(shard)
        for field in ("features", "purposes"):
            shard = deepcopy(self.shard)
            del shard["unit"][field]
            with self.assertRaisesRegex(trace.InvalidTrace, "expected object fields"):
                self.merge(shard)

    def test_compiled_inventory_must_match_every_compiler_test(self):
        artifact = {**self.shard["unit"], "executable": "/test-binary"}
        bundle = self.merge()
        with patch.object(trace, "execute", return_value="check: test\n") as execute:
            artifacts = trace.check_inventory(bundle, [artifact], self.root)
            self.assertEqual(artifacts, {trace.target_key(artifact): {"executable": "/test-binary", "directory": self.root}})
            self.assertEqual(execute.call_args.args[0], ["/test-binary", "--list", "--format=terse"])
        with patch.object(trace, "execute", return_value="check: test\nmissed: test\n"):
            with self.assertRaisesRegex(trace.InvalidTrace, "compiler missing.*missed"):
                trace.check_inventory(bundle, [artifact], self.root)
        with patch.object(trace, "execute", return_value=""):
            with self.assertRaisesRegex(trace.InvalidTrace, "absent from binary.*check"):
                trace.check_inventory(bundle, [artifact], self.root)
        with patch.object(trace, "execute", return_value="check: test\n"):
            with self.assertRaisesRegex(trace.InvalidTrace, "multiple compiled test artifacts"):
                trace.check_inventory(bundle, [artifact, {**artifact, "features": ["extra"]}], self.root)

    def test_duplicate_inventory_name_and_legacy_schema_fail(self):
        with self.assertRaisesRegex(trace.InvalidTrace, "duplicate test names"):
            trace.parse_inventory("check: test\ncheck: test\n")
        self.shard["version"] = 2
        with self.assertRaisesRegex(trace.InvalidTrace, "version must be 3"):
            self.merge()

    def test_no_test_name_or_substring_can_establish_a_run_passed(self):
        test = self.shard["tests"][0]
        artifacts = {trace.target_key(test): {"executable": "/captured-test-binary", "directory": self.root / "sample"}}
        failures = ["test check ... ignored\ntest result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s\n",
                    "test another_check ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n",
                    "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n"]
        for output in failures:
            with self.subTest(output=output), patch.object(trace, "execute", return_value=output):
                with self.assertRaises(trace.InvalidTrace):
                    trace.run_tests([test], artifacts)
        output = "running 1 test\ntest check ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.00s\n"
        with patch.object(trace, "execute", return_value=output) as execute:
            trace.run_tests([test], artifacts)
            self.assertEqual(execute.call_args.args,
                             (["/captured-test-binary", "check", "--exact", "--format", "pretty", "--color", "never"],
                              self.root / "sample"))
        with patch.object(trace, "execute") as execute:
            with self.assertRaisesRegex(trace.InvalidTrace, "compiled test artifact missing"):
                trace.run_tests([test], {})
            execute.assert_not_called()


if __name__ == "__main__":
    unittest.main()
