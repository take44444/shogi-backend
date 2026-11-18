#!/usr/bin/env python3
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

REPOSITORY = Path(__file__).resolve().parent.parent
MODEL = REPOSITORY / "scripts/traceability_model.rs"
MANIFEST = REPOSITORY / "tools/traceability/Cargo.toml"
TARGET = Path(os.environ.get("CARGO_TARGET_DIR", REPOSITORY / "target")).resolve()
ENV = {**os.environ, "CARGO_TARGET_DIR": str(TARGET)}


class DirectoryTraceabilityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        result = subprocess.run(["cargo", "build", "--quiet", "--locked", "--manifest-path", str(MANIFEST)],
                                capture_output=True, text=True, env=ENV)
        if result.returncode:
            raise RuntimeError(result.stderr)
        cls.collector = TARGET / "debug/traceability-collector"

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="directory-trace-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.package = self.root / "crates/sample"
        self.source = self.package / "src"
        self.source.mkdir(parents=True)
        (self.root / "Cargo.toml").write_text('[workspace]\nmembers=["crates/*"]\nresolver="3"\n')
        self.manifest = self.package / "Cargo.toml"
        self.manifest.write_text('[package]\nname="sample"\nversion="0.1.0"\nedition="2024"\n')
        self.lib = self.source / "lib.rs"
        self.lib.write_text(f'''pub fn implementation() -> i32 {{ 7 }}
#[cfg(test)]
#[path = {json.dumps(str(MODEL))}]
mod trace_support;
#[cfg(test)]
mod trace_metadata;
#[cfg(test)]
mod tests;
''')
        self.wire = self.source / "wire.rs"
        self.wire.write_text('''#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum WireCommand { Invoke }
fn router() { Router::new().route("/socket", get(upgrade)); }
''')
        self.metadata = self.source / "trace_metadata.rs"
        self.metadata.write_text('''#[path = "trace/component.rs"]
mod component;
#[path = "trace/features/demo.rs"]
mod demo;
#[path = "trace/contracts/behavior.rs"]
mod behavior;
''')
        self.component = self.source / "trace/component.rs"
        self.component.parent.mkdir()
        self.component.write_text('''use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "サンプルのコンポーネント。", notes: &["内部制約を別途検査する。"],
    external: &[trace::External { id: "frontend", title: "フロントエンド" }],
    operations: &[trace::Operation { contract: "sample/behavior", audience: trace::Audience::Frontend,
        surface: trace::Surface::WebSocket { path: "/socket", command: "invoke", source: trace::Source { path: "crates/sample/src/wire.rs", needle: "enum WireCommand" } },
        reason: "利用者の操作を受け付ける。" }],
    entrypoints: &[trace::EntryPoint { name: "操作の入口", source: trace::Source { path: "crates/sample/src/lib.rs", needle: "pub fn implementation" } }],
});
''')
        self.feature = self.source / "trace/features/demo.rs"
        self.feature.parent.mkdir()
        self.feature.write_text('''use crate::trace_support as trace;
trace::feature!(trace::Feature {
    id: "demo", title: "利用者の操作", source: trace::Source { path: "crates/sample/src/lib.rs", needle: "pub fn implementation" },
    flow: &["操作を送信する。"], guarantees: &[trace::Guarantee { id: "works", statement: "結果を取得する。", roots: &["sample/behavior"] }],
    limitations: &["通信環境を保証しない。"],
});
''')
        self.contract = self.source / "trace/contracts/behavior.rs"
        self.contract.parent.mkdir()
        self.contract.write_text('''use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "behavior", consumer: "frontend", provider: "sample", interface: trace::Interface::WebSocket,
    statement: "操作の結果を返す。", assumptions: &["有効な操作を受信する。"], limitations: &[],
    source: trace::Source { path: "crates/sample/src/lib.rs", needle: "pub fn implementation" },
    call: None, implementation: Some(trace::Source { path: "crates/sample/src/lib.rs", needle: "pub fn implementation" }),
    binding: None, requires: &[], gap: None,
});
''')
        self.internal = self.source / "tests.rs"
        self.internal.write_text('#[test]\nfn internal_case() { assert_eq!(crate::implementation(), 7); }\n')
        self.harness = self.package / "tests/contracts.rs"
        self.harness.parent.mkdir()
        self.harness.write_text(f'''#[path = {json.dumps(str(MODEL))}]
mod trace_support;
#[path = "contracts/behavior/tests.rs"]
mod behavior;
''')
        self.tests = self.package / "tests/contracts/behavior/tests.rs"
        self.tests.parent.mkdir(parents=True)
        self.tests.write_text('''use crate::trace_support as trace;
trace::boundary_case! {
    trace::BoundaryCase { evidence: &[trace::Evidence {
        contract: "sample/behavior", scope: "提供元の公開 API を検査する。", reason: "結果を直接検査する。",
        relationship: trace::Relationship::Asserts, observation: trace::Observation::ProviderBoundary,
        real: &["sample"], fake: &[],
    }] };
    #[test]
    fn selected() { assert_eq!(sample::implementation(), 7); }
}
''')
        self.lock()

    def lock(self):
        result = subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=self.root,
                                capture_output=True, text=True, env=ENV)
        self.assertEqual(result.returncode, 0, result.stderr)

    def collect(self, error=None, scan=False):
        result = subprocess.run([str(self.collector), *(["--scan"] if scan else []), str(self.root)],
                                capture_output=True, text=True, env=ENV)
        if error is not None:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn(error, result.stderr)
            return result
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return json.loads(result.stdout)

    def cargo(self, *args, error=None):
        result = subprocess.run(["cargo", "test", "--locked", *args], cwd=self.root,
                                capture_output=True, text=True, env=ENV)
        if error:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn(error, result.stderr)
        else:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    def append(self, path, source):
        path.write_text(path.read_text() + "\n" + source + "\n")

    def test_normalized_bundle_preserves_explicit_metadata_and_test_inventory(self):
        bundle = self.collect()
        self.assertEqual(bundle["version"], 2)
        self.assertEqual(set(bundle), {"version", "components", "features", "contracts", "tests", "inventories", "packages", "examples", "excluded_targets"})
        component, = bundle["components"]
        self.assertEqual((component["id"], component["package"], component["path"]), ("sample", "sample", "crates/sample"))
        self.assertEqual(component["record"], "crates/sample/src/trace/component.rs")
        self.assertEqual(component["operations"][0]["surface"]["kind"], "WebSocket")
        feature, = bundle["features"]
        self.assertEqual((feature["id"], feature["owner"]), ("demo", "sample"))
        self.assertEqual(feature["guarantees"][0]["roots"], ["sample/behavior"])
        contract, = bundle["contracts"]
        self.assertEqual((contract["id"], contract["owner"]), ("sample/behavior", "sample"))
        for field in ("call", "binding", "gap"):
            self.assertIsNone(contract[field])
        tests = {test["classification"]: test for test in bundle["tests"]}
        self.assertEqual(tests["internal"]["semantics"], {})
        self.assertEqual(tests["boundary"]["target"], {"kind": "test", "name": "contracts"})
        self.assertEqual(tests["boundary"]["semantics"]["evidence"][0]["observation"], "ProviderBoundary")
        self.assertEqual(tests["boundary"]["source"]["path"], "crates/sample/tests/contracts/behavior/tests.rs")
        listed = self.cargo("--", "--list").stdout
        listed_names = sorted(line.removesuffix(": test") for line in listed.splitlines() if line.endswith(": test"))
        self.assertEqual(listed_names, sorted(name for inventory in bundle["inventories"] for name in inventory["tests"]))
        self.cargo()
        self.assertFalse((self.root / "target").exists())

    def test_plain_src_and_internal_integration_tests_are_internal(self):
        internal = self.package / "tests/internal/local.rs"
        internal.parent.mkdir()
        internal.write_text('#[test]\nfn local() {}\n')
        (self.package / "tests/internal.rs").write_text('#[path="internal/local.rs"] mod local;\n')
        self.append(self.lib, '#[test]\nfn nearby() {}')
        tests = self.collect()["tests"]
        self.assertEqual({test["name"] for test in tests if test["classification"] == "internal"},
                         {"nearby", "tests::internal_case", "local::local"})
        self.cargo()

    def test_boundary_directory_does_not_implicitly_create_evidence(self):
        self.append(self.tests, '#[test]\nfn unreviewed() {}')
        self.collect(error="unclassified test;")

    def test_explicit_unreviewed_boundary_is_preserved(self):
        self.tests.write_text(self.tests.read_text().replace("Relationship::Asserts", "Relationship::Unreviewed"))
        boundary = next(test for test in self.collect()["tests"] if test["classification"] == "boundary")
        self.assertEqual(boundary["semantics"]["evidence"][0]["relationship"], "unreviewed")

    def test_boundary_wrapper_cannot_claim_unit_or_internal_integration_tests(self):
        self.internal.write_text(self.tests.read_text().replace("sample::implementation()", "crate::implementation()"))
        self.collect(error="boundary_case requires tests/contracts/ in an integration target")
        self.internal.write_text("")
        moved = self.package / "tests/internal/behavior.rs"
        moved.parent.mkdir()
        moved.write_text(self.tests.read_text())
        self.tests.unlink()
        self.harness.write_text(self.harness.read_text().replace("contracts/behavior/tests.rs", "internal/behavior.rs"))
        self.collect(error="boundary_case requires tests/contracts/ in an integration target")

    def test_contract_directory_loaded_by_library_is_not_integration_boundary(self):
        self.harness.unlink()
        self.append(self.lib, '#[cfg(test)]\n#[path="../tests/contracts/behavior/tests.rs"] mod boundary;')
        self.collect(error="boundary_case requires tests/contracts/ in an integration target")

    def test_plain_unclassified_integration_test_is_rejected(self):
        (self.package / "tests/forgotten.rs").write_text('#[test] fn forgotten() {}')
        self.collect(error="unclassified test;")

    def test_helpers_and_infrastructure_remain_distinct_from_evidence(self):
        self.append(self.lib, '#[cfg(test)] mod classified { use crate::trace_support; trace_support::helper! { "子プロセスとして呼び出す。"; #[test] fn probe() {} } trace_support::infrastructure! { "sql-policy", "SQL の方針を検査する。"; #[test] fn policy() {} } }')
        tests = self.collect()["tests"]
        helper = next(test for test in tests if test["classification"] == "helper")
        infrastructure = next(test for test in tests if test["classification"] == "infrastructure")
        self.assertEqual(helper["semantics"], {"reason": "子プロセスとして呼び出す。"})
        self.assertEqual(infrastructure["semantics"]["category"], "sql-policy")
        self.cargo()
        self.lib.write_text(self.lib.read_text().replace('"sql-policy"', '"catchall"'))
        self.collect(error="unsupported infrastructure category")

    def test_packages_without_metadata_are_components(self):
        package = self.root / "crates/worker"
        (package / "src").mkdir(parents=True)
        (package / "Cargo.toml").write_text('[package]\nname="worker"\nversion="0.1.0"\nedition="2024"\n')
        (package / "src/main.rs").write_text('fn main() {}\n#[test] fn tick() {}')
        self.lock()
        bundle = self.collect()
        worker = next(component for component in bundle["components"] if component["id"] == "worker")
        self.assertEqual(worker["description"], "worker")
        self.assertIsNone(worker["record"])
        self.assertEqual(worker["operations"], [])
        test = next(test for test in bundle["tests"] if test["package"] == "worker")
        self.assertEqual(test["classification"], "internal")
        self.assertEqual(test["target"], {"kind": "bin", "name": "worker"})
        self.cargo()

    def test_contract_references_are_global_and_not_inferred_from_test_directory(self):
        self.tests.write_text(self.tests.read_text().replace('contract: "sample/behavior"', 'contract: "other-package/provided"'))
        boundary = next(test for test in self.collect()["tests"] if test["classification"] == "boundary")
        self.assertEqual(boundary["semantics"]["evidence"][0]["contract"], "other-package/provided")

    def test_requires_and_complete_interaction_preserve_explicit_parties(self):
        self.contract.write_text(self.contract.read_text().replace('requires: &[]', 'requires: &[trace::Dependency { contract: "storage/read", reason: "結果を読み出す。" }]'))
        self.tests.write_text(self.tests.read_text().replace('Observation::ProviderBoundary', 'Observation::CompleteInteraction').replace('real: &["sample"]', 'real: &["frontend", "sample"]'))
        bundle = self.collect()
        self.assertEqual(bundle["contracts"][0]["requires"][0]["contract"], "storage/read")
        boundary = next(test for test in bundle["tests"] if test["classification"] == "boundary")
        self.assertEqual(boundary["semantics"]["evidence"][0]["observation"], "CompleteInteraction")
        self.cargo()

    def test_explicit_ids_must_match_declaration_paths(self):
        for path, old, new in ((self.feature, 'id: "demo"', 'id: "other"'),
                               (self.contract, 'id: "behavior"', 'id: "other"')):
            with self.subTest(path=path):
                original = path.read_text()
                path.write_text(original.replace(old, new))
                self.collect(error="declaration must live")
                path.write_text(original)

    def test_metadata_in_integration_target_is_rejected(self):
        self.append(self.harness, '#[path="../src/trace/component.rs"] mod metadata;')
        self.collect(error="metadata declarations require a lib or bin target")

    def test_duplicate_declarations_in_one_file_are_rejected(self):
        self.append(self.feature, self.feature.read_text().replace('use crate::trace_support as trace;', ''))
        self.collect(error="metadata file requires exactly one declaration")

    def test_metadata_compiled_in_two_targets_is_rejected(self):
        (self.source / "main.rs").write_text(f'''fn main() {{}}
#[cfg(test)]
#[path={json.dumps(str(MODEL))}]
mod trace_support;
#[cfg(test)]
mod trace_metadata;
''')
        self.collect(error="metadata file requires exactly one declaration")

    def test_feature_ids_are_unique_across_packages(self):
        package = self.root / "crates/other"
        (package / "src/trace/features").mkdir(parents=True)
        (package / "Cargo.toml").write_text('[package]\nname="other"\nversion="0.1.0"\nedition="2024"\n')
        (package / "src/lib.rs").write_text(f'''#[cfg(test)]
#[path={json.dumps(str(MODEL))}]
mod trace_support;
#[cfg(test)]
#[path="trace/features/demo.rs"]
mod demo;
''')
        (package / "src/trace/features/demo.rs").write_text(self.feature.read_text())
        self.lock()
        self.collect(error="duplicate feature demo")

    def test_literal_parser_covers_all_interfaces_audiences_and_surfaces(self):
        original = self.component.read_text()
        ws = 'trace::Surface::WebSocket { path: "/socket", command: "invoke", source: trace::Source { path: "crates/sample/src/wire.rs", needle: "enum WireCommand" } }'
        for kind, surface in (("OpenApi", 'trace::Surface::OpenApi { package: "sample", example: "schema", operation: "GET /sample" }'),
                              ("Grpc", 'trace::Surface::Grpc { path: "proto/service.proto", service: "Service", operation: "Invoke" }')):
            with self.subTest(kind=kind):
                self.component.write_text(original.replace(ws, surface))
                self.assertEqual(self.collect()["components"][0]["operations"][0]["surface"]["kind"], kind)
                self.cargo("--no-run")
        self.component.write_text(original)
        for audience in ("Frontend", "Internal", "Operator"):
            self.component.write_text(original.replace("Audience::Frontend", f"Audience::{audience}"))
            self.assertEqual(self.collect()["components"][0]["operations"][0]["audience"], audience)
        original_contract = self.contract.read_text()
        for interface in ("RustApi", "Port", "Http", "Grpc", "WebSocket", "External"):
            self.contract.write_text(original_contract.replace("Interface::WebSocket", f"Interface::{interface}"))
            self.assertEqual(self.collect()["contracts"][0]["interface"], interface)

    def test_missing_declaration_in_metadata_file_is_rejected(self):
        self.contract.write_text("")
        self.collect(error="metadata file requires exactly one declaration")

    def test_unknown_and_missing_semantic_fields_are_rejected(self):
        self.contract.write_text(self.contract.read_text().replace('gap: None', 'unknown: None'))
        self.collect(error="Contract requires exactly")

    def test_legacy_metadata_and_test_directories_are_rejected(self):
        legacy = self.source / "trace/old-feature"
        legacy.mkdir()
        self.collect(error="unsupported metadata path")
        legacy.rmdir()
        legacy = self.package / "tests/trace"
        legacy.mkdir()
        self.collect(error="legacy trace directory is unsupported")

    def test_old_case_wrapper_is_rejected_outside_scan(self):
        self.tests.write_text(self.tests.read_text().replace('trace::boundary_case!', 'trace::case!'))
        self.collect(error="unsupported item macro case")
        self.assertEqual(len(self.collect(scan=True)["tests"]), 2)

    def test_unreachable_metadata_boundary_and_internal_sources_are_rejected(self):
        for directory in (self.feature.parent, self.tests.parent, self.package / "tests/internal", self.source / "tests"):
            with self.subTest(directory=directory):
                directory.mkdir(exist_ok=True)
                path = directory / "forgotten.rs"
                path.write_text('#[test] fn forgotten() {}')
                self.collect(error="unreachable trace Rust file")
                path.unlink()

    def test_disconnected_source_tests_rs_files_are_rejected(self):
        for path in (self.internal, self.source / "nested/tests.rs"):
            with self.subTest(path=path):
                original = self.lib.read_text()
                if path == self.internal:
                    self.lib.write_text(original.replace("#[cfg(test)]\nmod tests;", ""))
                else:
                    path.parent.mkdir()
                    path.write_text("#[test] fn lost() {}")
                self.collect(error="unreachable trace Rust file")
                self.lib.write_text(original)
                if path != self.internal:
                    path.unlink()

    def test_websocket_enum_inventory_rejects_missing_extra_and_duplicate_commands(self):
        original = self.wire.read_text()
        self.wire.write_text(original.replace("Invoke }", "Invoke, Resign }"))
        self.collect(error='missing ["resign"]; extra []')
        self.wire.write_text(original)
        original_component = self.component.read_text()
        self.component.write_text(original_component.replace('command: "invoke"', 'command: "unknown"'))
        self.collect(error='missing ["invoke"]; extra ["unknown"]')
        operation = original_component.split("operations: &[")[1].split("],")[0]
        self.wire.write_text(original.replace("Invoke }", "Invoke, Resign }"))
        self.component.write_text(original_component.replace("operations: &[", "operations: &[" + operation.replace('command: "invoke"', 'command: "resign"').replace('needle: "enum WireCommand"', 'needle: "WireCommand"') + ","))
        self.assertEqual(len(self.collect()["components"][0]["operations"]), 2)
        self.wire.write_text(original)
        self.component.write_text(original_component.replace("operations: &[", "operations: &[" + operation + ","))
        self.collect(error="duplicate WebSocket command invoke")

    def test_websocket_serde_rules_are_explicitly_supported(self):
        original = self.wire.read_text()
        for source in (original.replace('"snake_case"', '"camelCase"'),
                       original.replace('tag = "type", ', ''),
                       original.replace('deny_unknown_fields', 'untagged'),
                       original.replace('rename_all = \"snake_case\"', 'rename_all(deserialize = \"snake_case\")'),
                       original.replace('Invoke }', '#[serde(rename = "invoke")] Invoke }'),
                       original.replace('Invoke }', '#[serde(alias = "call")] Invoke }')):
            with self.subTest(source=source):
                self.wire.write_text(source)
                self.collect(error="unsupported")
        self.wire.write_text(original.replace("Invoke }", "HTTPCall }"))
        self.component.write_text(self.component.read_text().replace('command: "invoke"', 'command: "h_t_t_p_call"'))
        self.collect()

    def upgrade_operation(self):
        self.component.write_text(self.component.read_text().replace("operations: &[", '''operations: &[trace::Operation {
            contract: "sample/behavior", audience: trace::Audience::Frontend,
            surface: trace::Surface::WebSocket { path: "/socket", command: "upgrade",
                source: trace::Source { path: "crates/sample/src/wire.rs", needle: ".route(" } },
            reason: "接続を開始する。" },'''))

    def test_websocket_upgrade_reconciles_the_anchored_route(self):
        self.upgrade_operation()
        self.assertEqual(len(self.collect()["components"][0]["operations"]), 2)
        original = self.wire.read_text()
        for source in (original.replace('"/socket"', '"/other"'),
                       original.replace('get(upgrade)', 'get(other)'),
                       original.replace('Router::new().route("/socket", get(upgrade));', '// .route("/socket", get(upgrade));\n')):
            with self.subTest(source=source):
                self.wire.write_text(source)
                self.collect(error="WebSocket upgrade must anchor")

    def test_websocket_command_source_must_anchor_an_enum(self):
        self.component.write_text(self.component.read_text().replace('needle: "enum WireCommand"', 'needle: "fn router"'))
        self.collect(error="must anchor a top-level serde command enum")

    def test_unknown_macro_inside_classified_directory_is_rejected(self):
        self.append(self.tests, 'macro_rules! generated { () => {}; }\ngenerated!();')
        self.collect(error="unsupported item macro")

    def test_unsupported_cfg_inside_classified_directory_is_rejected(self):
        self.append(self.tests, '#[cfg(any())]\nmod conditional {}')
        self.collect(error="unsupported cfg inside trace directory")

    def test_conditional_modules_outside_classified_directories_preserve_inventory_check(self):
        self.append(self.lib, '#[cfg(any())] mod disabled;\n#[cfg(all())] mod active { #[test] fn hidden() {} }')
        parsed = {test["name"] for test in self.collect()["tests"]}
        self.assertNotIn("active::hidden", parsed)
        self.assertIn("active::hidden: test", self.cargo("--", "--list").stdout)

    def test_test_attributes_inside_wrappers_are_required(self):
        self.tests.write_text(self.tests.read_text().replace('trace::boundary_case!', '#[cfg(test)]\ntrace::boundary_case!'))
        self.collect(error="put test attributes inside the transparent wrapper")

    def test_real_targets_typecheck_semantics(self):
        self.tests.write_text(self.tests.read_text().replace('trace::Relationship::Asserts', '"asserts"'))
        self.collect()
        self.cargo("--no-run", error="mismatched types")

    def test_nested_module_names_and_private_internal_include_are_preserved(self):
        self.internal.write_text('mod nested { #[test] fn selected() {} }\n#[test] fn selected() { assert_eq!(private(), 7); }\nmod child;')
        self.lib.write_text(self.lib.read_text().replace('mod tests;', 'mod tests { fn private() -> i32 { 7 } include!("tests.rs"); }'))
        (self.source / "child.rs").write_text('#[test] fn nested() { assert_eq!(super::private(), 7); }')
        names = {test["name"] for test in self.collect()["tests"]}
        self.assertTrue({"tests::nested::selected", "tests::selected", "tests::child::nested"} <= names)
        self.cargo()

    def test_explicit_module_paths_resolve_children_from_file_parent(self):
        self.append(self.tests, 'mod child;')
        (self.tests.parent / "child.rs").write_text('use crate::trace_support; trace_support::helper! { "補助検査を起動する。"; #[test] fn nested() {} }')
        self.assertIn("behavior::child::nested", {test["name"] for test in self.collect()["tests"]})
        self.cargo()

    def test_duplicate_and_cyclic_literal_includes_are_rejected(self):
        self.append(self.lib, 'include!("tests.rs");')
        self.collect(error="duplicate Rust source in target")
        self.lib.write_text(self.lib.read_text().replace('include!("tests.rs");', ''))
        self.append(self.internal, 'include!("tests.rs");')
        self.collect(error="cyclic Rust module")

    def test_computed_and_outside_repository_includes_are_rejected(self):
        self.append(self.internal, 'include!(concat!("unused", ".rs"));')
        self.collect(error="include! requires a literal Rust source path")
        external = tempfile.TemporaryDirectory(prefix="outside-trace-")
        self.addCleanup(external.cleanup)
        source = Path(external.name) / "escape.rs"
        source.write_text('#[test] fn escape() {}')
        relative = json.dumps(os.path.relpath(source, self.internal.parent))
        self.internal.write_text(f'include!({relative});')
        self.collect(error="Rust source outside repository")

    def test_sources_in_another_package_cannot_own_tests(self):
        nested = self.root / "unregistered/src/tests.rs"
        nested.parent.mkdir(parents=True)
        nested.write_text(self.internal.read_text())
        self.internal.unlink()
        relative = json.dumps(os.path.relpath(nested, self.source))
        self.lib.write_text(self.lib.read_text().replace('mod tests;', f'#[path={relative}] mod tests;'))
        self.collect(error="test source belongs to another package")

    def test_empty_test_targets_remain_in_inventory(self):
        (self.package / "tests/empty.rs").write_text("")
        inventory = next(item for item in self.collect()["inventories"] if item["target"] == {"kind": "test", "name": "empty"})
        self.assertEqual(inventory["tests"], [])
        self.cargo("--", "--list")

    def test_test_false_and_required_feature_policy_are_preserved(self):
        (self.package / "tests/excluded.rs").write_text('compile_error!("must not compile");')
        self.append(self.manifest, '[[test]]\nname="excluded"\ntest=false')
        bundle = self.collect()
        self.assertEqual(bundle["excluded_targets"][0]["target"], {"kind": "test", "name": "excluded"})
        self.manifest.write_text(self.manifest.read_text().replace('test=false', 'required-features=["extra"]\n[features]\nextra=[]'))
        self.collect(error="required-features targets are unsupported")

    def example(self, declaration=True, kind="Program", body="fn main() {}"):
        directory = self.package / "examples"
        directory.mkdir(exist_ok=True)
        path = directory / "sample.rs"
        marker = f'''trace_support::example!(trace_support::Example {{ kind: trace_support::ExampleKind::{kind}, reason: "公開仕様または利用例を提供する。" }});'''
        path.write_text(f'#[path={json.dumps(str(MODEL))}] mod trace_support;\n' + (marker if declaration else '') + '\n' + body)
        return path, marker

    def test_examples_require_one_top_level_unconditional_declaration(self):
        path, marker = self.example(False)
        self.collect(error="exactly one top-level example! declaration")
        path, marker = self.example()
        source = path.read_text()
        for invalid in (source + marker, source.replace(marker, 'mod nested { ' + marker + ' }'),
                        source.replace(marker, '#[cfg(any())]\n' + marker), '#![cfg(any())]\n' + source):
            with self.subTest(source=invalid):
                path.write_text(invalid)
                self.collect(error="top-level example! declaration")
        path.write_text(source.replace('reason:', 'unknown:'))
        self.collect(error="Example requires exactly")

    def test_program_and_openapi_examples_are_collected_without_execution(self):
        path, _ = self.example(body='fn main() { panic!("must not execute"); }')
        self.assertEqual(self.collect()["examples"][0]["kind"], "Program")
        self.cargo("--examples", "--no-run")
        path.write_text(path.read_text().replace('ExampleKind::Program', 'ExampleKind::OpenApi'))
        self.assertEqual(self.collect()["examples"][0]["kind"], "OpenApi")
        path.write_text(path.read_text().replace('"公開仕様または利用例を提供する。"', '""'))
        self.collect(error="expected nonempty single-line string")

    def test_required_feature_examples_are_rejected(self):
        self.example()
        self.append(self.manifest, '[features]\nmanual=[]\n[[example]]\nname="sample"\nrequired-features=["manual"]')
        self.collect(error="required-features examples are unsupported")

    def test_scan_preserves_utf8_byte_ranges_and_legacy_tokens_without_compiling(self):
        source = self.tests.read_text().replace('#[test]', '#[doc = "海"] #[test]').replace('assert_eq!(sample::implementation(), 7);', 'compile_error!("scan does not compile");')
        source = source.replace('trace::boundary_case!', 'trace::case!').replace('trace::BoundaryCase { evidence:', 'UnknownSchema { old_field:')
        self.tests.write_text(source)
        test = next(test for test in self.collect(scan=True)["tests"] if test["name"] == "behavior::selected")
        data = source.encode()
        self.assertEqual(data[test["span"]["start"]:test["span"]["end"]].decode(), test["function_source"])
        self.assertEqual(data[test["body_span"]["start"]:test["body_span"]["end"]].decode(), test["body_source"])
        self.assertTrue(data[test["outer_span"]["start"]:test["outer_span"]["end"]].decode().startswith('trace::case!'))
        self.assertIn('#[doc = "海"] #[test]', test["function_source"])
        self.assertIn("compile_error", test["function_tokens"])

    def test_library_crate_types_remain_in_inventory(self):
        self.append(self.manifest, '[lib]\ncrate-type=["rlib"]')
        self.assertTrue(any(item["target"] == {"kind": "lib"} for item in self.collect()["inventories"]))


if __name__ == "__main__":
    unittest.main()
