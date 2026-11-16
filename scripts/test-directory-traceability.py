#!/usr/bin/env python3
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

REPOSITORY = Path(__file__).resolve().parent.parent
CHECKER = REPOSITORY / "scripts/traceability.py"
MODEL = REPOSITORY / "scripts/traceability_model.rs"
os.environ["CARGO_TARGET_DIR"] = str(Path(os.environ.get("CARGO_TARGET_DIR", REPOSITORY / "target")).resolve())
spec = importlib.util.spec_from_file_location("traceability", CHECKER)
traceability = importlib.util.module_from_spec(spec)
spec.loader.exec_module(traceability)


class DirectoryTraceabilityTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="directory-trace-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "crates/sample/src"
        self.feature = self.source / "trace/demo"
        self.contract = self.feature / "behavior"
        self.contract.mkdir(parents=True)
        (self.root / "Cargo.toml").write_text('[workspace]\nmembers = ["crates/*"]\nresolver = "3"\n')
        (self.source.parent / "Cargo.toml").write_text('[package]\nname = "sample"\nversion = "0.1.0"\nedition = "2024"\n')
        self.lib = self.source / "lib.rs"
        self.lib.write_text(f'''pub fn implementation() {{}}
#[cfg(test)]
#[path = {json.dumps(str(MODEL))}]
mod trace_support;
#[cfg(test)]
#[path = "trace/demo/feature.rs"]
mod feature;
#[cfg(test)]
#[path = "trace/demo/behavior/contract.rs"]
mod contract;
#[cfg(test)]
#[path = "trace/demo/behavior/tests.rs"]
mod cases;
''')
        self.write_feature(self.feature, "Demo")
        (self.contract / "contract.rs").write_text('''use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Calls the implementation", source: trace::Source { path: "crates/sample/src/lib.rs", needle: "pub fn implementation() {}" }, gap: None,
});
''')
        self.tests = self.contract / "tests.rs"
        self.tests.write_text('''use crate::trace_support as trace;
trace::case! {
    trace::Case { scope: "Pure local function", reason: "Reviewed direct call", relationship: trace::Relationship::Asserts, also: &[] };
    #[test]
    fn selected() { crate::implementation(); }
}
''')
        (self.root / "proto").mkdir()
        (self.root / "proto/service.proto").write_text("service Service { rpc Invoke(Request) returns (Response); }\n")
        result = subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=self.root, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)

    def write_feature(self, directory, title):
        directory.mkdir(exist_ok=True)
        (directory / "feature.rs").write_text('''use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "TITLE", source: trace::Source { path: "crates/sample/src/lib.rs", needle: "pub fn implementation() {}" },
    flow: &["Call implementation"],
    guarantees: &[trace::Guarantee { id: "works", statement: "Calls work", contracts: &["behavior"] }],
    limitations: &["No external services"],
    entrypoints: &[trace::EntryPoint { name: "TITLE entry", source: trace::Source { path: "crates/sample/src/lib.rs", needle: "pub fn implementation() {}" } }],
    surfaces: SURFACES,
});
'''.replace("TITLE", title).replace("SURFACES", '&[trace::Surface::Grpc { path: "proto/service.proto", service: "Service", operation: "Invoke" }]' if title == "Demo" else '&[]'))

    def cli(self, command="render", *, error=None):
        result = subprocess.run([sys.executable, str(CHECKER), command, "--root", str(self.root)],
                                capture_output=True, text=True, env=os.environ.copy())
        if error is None:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn(error, result.stderr)
        return result

    def graph(self):
        return traceability.load_traces(self.root)[0][1]

    def append(self, path, source):
        path.write_text(path.read_text() + "\n" + source + "\n")

    def example_source(self, kind="OpenApi", reason="Exports the public schema", main=None):
        if main is None:
            main = 'fn main() { println!(r#"{{"paths":{{"/new":{{"get":{{}}}}}}}}"#); }'
        return f'''#[path = {json.dumps(str(MODEL))}]
mod trace_support;
trace_support::example!(trace_support::Example {{
    kind: trace_support::ExampleKind::{kind}, reason: {json.dumps(reason)},
}});
{main}
'''

    def test_normal_cli_render_check_and_exact_run(self):
        self.cli()
        self.cli("check")
        result = self.cli("run")
        self.assertIn("PASS demo/", result.stdout)
        self.assertIn("cases::selected", result.stdout)
        graph = self.graph()
        self.assertEqual(graph["tests"][0]["name"], "cases::selected")
        self.assertEqual(graph["tests"][0]["source"]["path"], "crates/sample/src/trace/demo/behavior/tests.rs")

    def test_new_ordinary_contract_test_is_unreviewed(self):
        self.append(self.tests, "#[test]\nfn automatically_discovered() {}")
        self.cli()
        self.cli("check")
        graph = self.graph()
        self.assertEqual(len(graph["tests"]), 2)
        relationships = {edge["relationship"] for edge in graph["contracts"][0]["evidence"]}
        self.assertEqual(relationships, {"asserts", "unreviewed"})
        document = (self.root / "docs/reference/traceability/demo.md").read_text()
        self.assertIn("automatically_discovered", document)
        self.assertIn("所属のみ・未レビュー", document)

    def test_unreviewed_only_contract_requires_gap(self):
        self.tests.write_text("#[test]\nfn ordinary() {}\n")
        self.cli(error="evidence or explicit gap required")
        path = self.contract / "contract.rs"
        path.write_text(path.read_text().replace("gap: None", 'gap: Some("Assertions need review")'))
        self.cli()

    def test_duplicate_function_names_use_actual_module_and_span(self):
        self.append(self.tests, "mod nested { #[test]\nfn selected() {} }")
        self.cli()
        self.cli("check")
        names = {test["name"] for test in self.graph()["tests"]}
        self.assertEqual(names, {"cases::selected", "cases::nested::selected"})

    def test_original_scope_test_must_be_classified(self):
        self.append(self.lib, "#[test]\nfn unclassified() {}")
        self.cli(error="unclassified test;")

    def test_explicit_helper_is_accounted_without_becoming_evidence(self):
        self.append(self.lib, 'trace_support::helper! { "Process harness"; #[test] fn probe() {} }')
        self.cli()
        self.assertEqual(len(self.graph()["tests"]), 1)
        self.assertIn("Process harness", (self.root / "docs/reference/feature-traceability.md").read_text())

    def test_infrastructure_requires_an_explicit_category_and_reason(self):
        self.append(self.lib, 'trace_support::infrastructure! { "sql-policy", "Checks the SQL policy"; #[test] fn policy() {} }')
        self.cli()
        index = (self.root / "docs/reference/feature-traceability.md").read_text()
        self.assertIn("Checks the SQL policy", index)
        self.assertIn("sql-policy", index)
        self.assertIn("sample / lib / (lib) / policy", index)
        self.lib.write_text(self.lib.read_text().replace('"sql-policy"', '"catchall"'))
        self.cli(error="unsupported infrastructure category")

    def test_new_feature_does_not_require_json_registration(self):
        other = self.source / "trace/second"
        self.write_feature(other, "Second")
        (other / "behavior").mkdir()
        (other / "behavior/contract.rs").write_text((self.contract / "contract.rs").read_text().replace("gap: None", 'gap: Some("No reviewed evidence")'))
        self.append(self.lib, '#[cfg(test)]\n#[path = "trace/second/feature.rs"]\nmod second_feature;\n#[cfg(test)]\n#[path = "trace/second/behavior/contract.rs"]\nmod second_contract;')
        self.cli()
        self.cli("check")
        self.assertTrue((self.root / "docs/reference/traceability/second.md").is_file())

    def test_unknown_cross_feature_evidence_is_rejected(self):
        self.tests.write_text(self.tests.read_text().replace('also: &[]', 'also: &[trace::Evidence { feature: "absent", contract: "behavior", scope: "Remote feature", relationship: trace::Relationship::Exercises, reason: "Other edge" }]'))
        self.cli(error="unknown cross-feature/contract reference absent/behavior")

    def test_unmounted_new_feature_directory_is_rejected(self):
        other = self.source / "trace/new-feature"
        other.mkdir()
        (other / "feature.rs").write_text("")
        self.cli(error="unreachable trace Rust file")

    def test_empty_new_feature_directory_is_rejected(self):
        (self.source / "trace/new-feature").mkdir()
        self.cli(error="requires exactly one reachable feature.rs declaration")

    def test_unreachable_file_inside_known_feature_is_rejected(self):
        (self.contract / "forgotten.rs").write_text("#[test] fn forgotten() {}\n")
        self.cli(error="unreachable trace Rust file")

    def test_unknown_macro_in_classified_scope_is_rejected(self):
        self.append(self.lib, "macro_rules! generated { () => { #[test] fn hidden() {} }; }\ngenerated!();")
        self.cli(error="global classification inventory mismatch")

    def test_disabled_external_module_outside_trace_directory_is_ignored(self):
        self.append(self.lib, '#[cfg(any())]\nmod platform_disabled;')
        self.cli()
        self.cli("check")
        self.assertEqual([test["name"] for test in self.graph()["tests"]], ["cases::selected"])

    def test_active_unsupported_cfg_outside_trace_directory_is_caught_by_inventory(self):
        self.append(self.lib, '#[cfg(all())]\nmod conditional { #[test] fn hidden() {} }')
        self.cli(error="global classification inventory mismatch")

    def test_unsupported_cfg_inside_trace_directory_is_rejected_even_if_empty(self):
        self.append(self.tests, '#[cfg(any())]\nmod conditional {}')
        self.cli(error="unsupported cfg inside trace directory")

    def test_unknown_macro_inside_trace_directory_is_rejected(self):
        self.append(self.tests, 'macro_rules! generated { () => {}; }\ngenerated!();')
        self.cli(error="unsupported item macro")

    def test_semantics_are_typechecked_by_real_target(self):
        self.tests.write_text(self.tests.read_text().replace("trace::Relationship::Asserts", '"asserts"'))
        self.cli(error="mismatched types")

    def test_binary_contract_tests_are_discovered(self):
        package = self.root / "apps/worker"
        source = package / "src"
        contract = source / "trace/demo/behavior"
        contract.mkdir(parents=True)
        (package / "Cargo.toml").write_text('[package]\nname = "worker"\nversion = "0.1.0"\nedition = "2024"\n')
        (source / "main.rs").write_text(f'''fn main() {{}}
#[cfg(test)]
#[path = {json.dumps(str(MODEL))}]
mod trace_support;
#[cfg(test)]
#[path = "trace/demo/behavior/tests.rs"]
mod cases;
''')
        (contract / "tests.rs").write_text("#[test]\nfn bin_case() {}\n")
        (self.root / "Cargo.toml").write_text('[workspace]\nmembers = ["crates/*", "apps/*"]\nresolver = "3"\n')
        subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=self.root, check=True, capture_output=True)
        self.cli()
        graphs = traceability.load_traces(self.root)
        test = next(t for t in graphs[0][1]["tests"] if t["package"] == "worker")
        self.assertEqual(test["target"], {"kind": "bin", "name": "worker"})
        self.cli("run")


    def test_literal_include_preserves_original_namespace_and_private_helpers(self):
        self.lib.write_text(self.lib.read_text().replace('#[cfg(test)]\n#[path = "trace/demo/behavior/tests.rs"]\nmod cases;', '#[cfg(test)]\nmod original { fn private() -> i32 { 7 } include!("trace/demo/behavior/tests.rs"); }'))
        self.tests.write_text(self.tests.read_text().replace('crate::implementation();', 'assert_eq!(private(), 7);'))
        self.cli()
        self.cli("run")
        self.assertEqual(self.graph()["tests"][0]["name"], "original::selected")

    def test_literal_include_resolves_child_modules_from_included_file(self):
        self.lib.write_text(self.lib.read_text().replace('#[cfg(test)]\n#[path = "trace/demo/behavior/tests.rs"]\nmod cases;', '#[cfg(test)]\nmod original { fn private() -> i32 { 7 } include!("trace/demo/behavior/tests.rs"); }'))
        self.append(self.tests, 'mod child;')
        (self.contract / "child.rs").write_text('#[test]\nfn nested() { assert_eq!(super::private(), 7); }\n')
        self.cli()
        self.cli("run")
        self.assertEqual({test["name"] for test in self.graph()["tests"]},
                         {"original::selected", "original::child::nested"})
        (self.contract / "orphan.rs").write_text('#[test] fn hidden() {}\n')
        self.cli(error="unreachable trace Rust file")

    def test_explicit_module_path_resolves_child_modules_from_file_parent(self):
        self.lib.write_text(self.lib.read_text().replace('mod cases;', 'mod original;'))
        self.append(self.tests, 'fn private() -> i32 { 7 }\nmod child;')
        (self.contract / "child.rs").write_text('#[test]\nfn nested() { assert_eq!(super::private(), 7); }\n')
        self.cli()
        self.cli("run")
        self.assertEqual({test["name"] for test in self.graph()["tests"]},
                         {"original::selected", "original::child::nested"})
        (self.contract / "orphan.rs").write_text('#[test] fn hidden() {}\n')
        self.cli(error="unreachable trace Rust file")

    def test_duplicate_and_cyclic_literal_includes_are_rejected(self):
        self.append(self.lib, 'include!("trace/demo/behavior/tests.rs");')
        self.cli(error="duplicate Rust source in target")
        self.lib.write_text(self.lib.read_text().replace('include!("trace/demo/behavior/tests.rs");', ''))
        self.append(self.tests, 'include!("tests.rs");')
        self.cli(error="cyclic Rust module")

    def test_computed_and_outside_repository_includes_are_rejected(self):
        self.append(self.tests, 'include!(concat!("unused", ".rs"));')
        self.cli(error="include! requires a literal Rust source path")
        external = tempfile.TemporaryDirectory(prefix="outside-trace-")
        self.addCleanup(external.cleanup)
        source = Path(external.name) / "escape.rs"
        source.write_text("#[test] fn escape() {}")
        path = json.dumps(os.path.relpath(source, self.tests.parent))
        self.tests.write_text(self.tests.read_text().replace('include!(concat!("unused", ".rs"));', f'include!({path});'))
        self.cli(error="outside repository")

    def second_feature(self):
        other = self.source / "trace/second"
        self.write_feature(other, "Second")
        (other / "behavior").mkdir()
        (other / "behavior/contract.rs").write_text((self.contract / "contract.rs").read_text().replace("gap: None", 'gap: Some("No reviewed evidence")'))
        self.append(self.lib, '#[cfg(test)]\n#[path = "trace/second/feature.rs"]\nmod second_feature;\n#[cfg(test)]\n#[path = "trace/second/behavior/contract.rs"]\nmod second_contract;')
        return other

    def test_cross_feature_evidence_preserves_distinct_scopes_and_runs_once(self):
        self.second_feature()
        self.tests.write_text(self.tests.read_text().replace('also: &[]', 'also: &[trace::Evidence { feature: "second", contract: "behavior", scope: "Second feature scope", relationship: trace::Relationship::Exercises, reason: "Shared execution" }]'))
        self.cli()
        traces = traceability.load_traces(self.root)
        self.assertEqual([graph["tests"][0]["scope"] for _, graph, _ in traces], ["Pure local function", "Second feature scope"])
        self.assertEqual(len(list(traceability.mapped_tests(traces))), 1)
        self.assertEqual(self.cli("run").stdout.count("PASS "), 1)

    def test_duplicate_evidence_edges_and_conflicting_feature_scopes_are_rejected(self):
        original = self.tests.read_text()
        edge = 'trace::Evidence { feature: "demo", contract: "behavior", scope: "Pure local function", relationship: trace::Relationship::Exercises, reason: "Repeated edge" }'
        self.tests.write_text(original.replace('also: &[]', f'also: &[{edge}]'))
        self.cli(error="duplicate evidence edge")
        other = self.contract.parent / "other"
        other.mkdir()
        (other / "contract.rs").write_text((self.contract / "contract.rs").read_text())
        self.append(self.lib, '#[cfg(test)]\n#[path = "trace/demo/other/contract.rs"]\nmod other_contract;')
        edge = edge.replace('contract: "behavior"', 'contract: "other"').replace('scope: "Pure local function"', 'scope: "Conflicting scope"')
        self.tests.write_text(original.replace('also: &[]', f'also: &[{edge}]'))
        self.cli(error="conflicting feature scopes")

    def test_new_unclassified_integration_target_and_workspace_package_are_rejected(self):
        target = self.source.parent / "tests/forgotten.rs"
        target.parent.mkdir(exist_ok=True)
        target.write_text("#[test] fn forgotten() {}")
        self.cli(error="unclassified test;")
        target.unlink()
        package = self.root / "crates/new-member"
        (package / "src").mkdir(parents=True)
        (package / "Cargo.toml").write_text('[package]\nname="new-member"\nversion="0.1.0"\nedition="2024"\n')
        (package / "src/lib.rs").write_text("#[test] fn forgotten() {}")
        subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=self.root, check=True, capture_output=True)
        self.cli(error="unclassified test;")

    def test_zero_test_targets_remain_in_the_global_inventory(self):
        target = self.source.parent / "tests/empty.rs"
        target.parent.mkdir(exist_ok=True)
        target.write_text("")
        self.cli()
        bundle = traceability.directory_traces(self.root)
        target = next(item for item in bundle["inventories"] if item["target"] == {"kind": "test", "name": "empty"})
        self.assertEqual(target["tests"], [])
        self.cli("check")

    def test_test_false_is_honored_and_required_features_are_explicitly_rejected(self):
        manifest = self.source.parent / "Cargo.toml"
        target = self.source.parent / "tests/excluded.rs"
        target.parent.mkdir(exist_ok=True)
        target.write_text('#[cfg(test)] compile_error!("must not compile");')
        self.append(manifest, '[[test]]\nname="excluded"\ntest=false')
        self.cli()
        bundle = traceability.directory_traces(self.root)
        self.assertEqual(bundle["excluded_targets"][0]["target"], {"kind": "test", "name": "excluded"})
        manifest.write_text(manifest.read_text().replace('test=false', 'required-features=["extra"]\n[features]\nextra=[]'))
        self.cli(error="required-features targets are unsupported")

    def test_new_proto_file_service_and_rpc_are_discovered(self):
        directory = self.source.parent / "proto"
        directory.mkdir()
        path = directory / "new.proto"
        path.write_text('service NewService { rpc NewRpc(Request) returns (Response); }')
        self.cli(error="NewService")
        path.unlink()
        self.append(self.root / "proto/service.proto", 'service OtherService { rpc NewRpc(Request) returns (Response); }')
        self.cli(error="OtherService")
        path = self.root / "proto/service.proto"
        path.write_text('service Service { rpc Invoke(Request) returns (Response); rpc NewRpc(Request) returns (Response); }')
        self.cli(error="NewRpc")

    def test_new_exporter_and_changed_openapi_operations_are_discovered(self):
        examples = self.source.parent / "examples"
        examples.mkdir()
        exporter = examples / "schema.rs"
        exporter.write_text('fn main() { println!(r#"{{"paths":{{"/new":{{"get":{{}}}}}}}}"#); }')
        self.cli("check", error="exactly one top-level example! declaration")
        exporter.write_text(self.example_source())
        self.cli(error="OpenAPI exporter set mismatch")
        feature = self.feature / "feature.rs"
        feature.write_text(feature.read_text().replace('surfaces: &[', 'surfaces: &[trace::Surface::OpenApi { package: "sample", example: "schema", operation: "GET /new" },'))
        self.cli()
        exporter.write_text(exporter.read_text().replace('get', 'post'))
        self.cli(error="OpenAPI operation set mismatch")

    def test_reasoned_program_examples_are_compiled_without_execution(self):
        examples = self.source.parent / "examples"
        examples.mkdir()
        program = examples / "ordinary.rs"
        program.write_text(self.example_source("Program", "Needs an external service", 'fn main() { panic!("must not execute"); }'))
        self.cli()
        self.cli("check")
        program.write_text(self.example_source("Program", ""))
        self.cli(error="expected nonempty single-line string")
        program.write_text(self.example_source("Program", "Needs an external service", 'enum Impostor { Program }\nfn main() {}').replace('trace_support::ExampleKind::Program', 'Impostor::Program'))
        self.cli(error="mismatched types")

    def test_example_declarations_must_be_unique_top_level_unconditional_and_well_formed(self):
        examples = self.source.parent / "examples"
        examples.mkdir()
        program = examples / "ordinary.rs"
        source = self.example_source("Program", "Runs a demonstration", 'fn main() {}')
        marker = source[source.index('trace_support::example!'):source.index('fn main')]
        for invalid in (source + marker, source.replace(marker, 'mod nested { ' + marker + ' }\n'),
                        source.replace(marker, '#[cfg(any())]\n' + marker), '#![cfg(any())]\n' + source):
            with self.subTest(source=invalid):
                program.write_text(invalid)
                self.cli(error="top-level example! declaration")
        program.write_text(source.replace('reason:', 'unknown:'))
        self.cli(error="Example requires exactly")

    def test_required_feature_examples_are_rejected_by_the_default_feature_policy(self):
        examples = self.source.parent / "examples"
        examples.mkdir()
        (examples / "ordinary.rs").write_text(self.example_source("Program", "Needs optional functionality", 'fn main() {}'))
        self.append(self.source.parent / "Cargo.toml", '[features]\nmanual=[]\n[[example]]\nname="ordinary"\nrequired-features=["manual"]')
        self.cli("check", error="required-features examples are unsupported")

    def test_duplicate_surface_owners_are_rejected(self):
        feature = self.feature / "feature.rs"
        surface = 'trace::Surface::Grpc { path: "proto/service.proto", service: "Service", operation: "Invoke" }'
        feature.write_text(feature.read_text().replace('surfaces: &[', f'surfaces: &[{surface},'))
        self.cli(error="duplicate gRPC operation owner")


    def test_scan_exports_utf8_byte_ranges_and_tokens_without_typechecking_or_execution(self):
        source = self.tests.read_text().replace('#[test]', '#[doc = "海"] #[test]').replace('crate::implementation();', 'compile_error!("scan does not compile or execute");')
        source = source.replace('relationship: trace::Relationship::Asserts', 'relationship: "old or invalid schema"')
        self.tests.write_text(source)
        result = subprocess.run(["cargo", "run", "--quiet", "--locked", "--manifest-path", str(REPOSITORY / "tools/traceability/Cargo.toml"), "--", "--scan", str(self.root)], capture_output=True, text=True, env=os.environ.copy())
        self.assertEqual(result.returncode, 0, result.stderr)
        test = json.loads(result.stdout)["tests"][0]
        data = source.encode()
        self.assertEqual(data[test["span"]["start"]:test["span"]["end"]].decode(), test["function_source"])
        outer = data[test["outer_span"]["start"]:test["outer_span"]["end"]].decode()
        self.assertTrue(outer.startswith("trace::case!"))
        self.assertIn('#[doc = "海"] #[test]', test["function_source"])
        self.assertIn("compile_error", test["function_tokens"])
        self.assertEqual(test["name"], "cases::selected")

    def test_library_crate_types_are_included_in_global_discovery(self):
        self.append(self.source.parent / "Cargo.toml", '[lib]\ncrate-type=["rlib"]')
        self.cli()
        bundle = traceability.directory_traces(self.root)
        self.assertEqual(bundle["inventories"][0]["target"], {"kind": "lib"})


    def test_nested_trace_paths_cannot_claim_ownership_or_hide_from_canonical_discovery(self):
        nested = self.source / "actor/trace/demo/behavior/tests.rs"
        nested.parent.mkdir(parents=True)
        nested.write_text(self.tests.read_text())
        self.tests.unlink()
        self.lib.write_text(self.lib.read_text().replace('trace/demo/behavior/tests.rs', 'actor/trace/demo/behavior/tests.rs'))
        self.cli(error="unclassified test;")

    def test_trace_directory_in_another_package_cannot_own_this_targets_test(self):
        nested = self.root / "unregistered/src/trace/demo/behavior/tests.rs"
        nested.parent.mkdir(parents=True)
        nested.write_text(self.tests.read_text())
        self.tests.unlink()
        path = json.dumps(os.path.relpath(nested, self.source))
        self.lib.write_text(self.lib.read_text().replace('#[path = "trace/demo/behavior/tests.rs"]\nmod cases;', f'#[path = {path}]\nmod cases;'))
        self.cli(error="unclassified test;")


if __name__ == "__main__":
    unittest.main()
