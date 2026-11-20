#!/usr/bin/env python3
import json
import os
from pathlib import Path
import tempfile
import unittest

import traceability as trace
from traceability_viewer import write_viewer

REPOSITORY = Path(__file__).resolve().parent.parent
TOOLS = Path(os.environ.get("TRACEABILITY_TEST_TOOL_ROOT", REPOSITORY)).resolve()


class CompilerPipelineTests(unittest.TestCase):
    def check_macro_identities(self, generated, target=None, *, preserve_input_span=False, declarative=False):
        with tempfile.TemporaryDirectory(prefix="compiler-trace-ambiguous-macro-") as directory:
            root = Path(directory)
            (root / "tools").mkdir()
            (root / "tools/traceability").symlink_to(TOOLS / "tools/traceability", target_is_directory=True)
            test = ('#[traceability::targets(' + target + ')] #[test] fn exact() {}') if target else '#[test] fn unrelated() {}'
            macro = 'use proc_macro::TokenStream;\n#[proc_macro] pub fn generate(_:TokenStream)->TokenStream { ' + json.dumps(generated) + '.parse().unwrap() }\n'
            if preserve_input_span:
                macro = '''use proc_macro::{TokenStream, TokenTree, Span, Group};
fn assign(tokens: TokenStream, span: Span) -> TokenStream {
    tokens.into_iter().map(|token| match token {
        TokenTree::Group(group) => {
            let mut result = Group::new(group.delimiter(), assign(group.stream(), span));
            result.set_span(span);
            TokenTree::Group(result)
        }
        mut other => { other.set_span(span); other }
    }).collect()
}
#[proc_macro] pub fn generate(input: TokenStream) -> TokenStream {
    let span = input.into_iter().next().unwrap().span();
    assign(''' + json.dumps(generated) + '''.parse().unwrap(), span)
}
'''
            invocation = 'generator::generate!(origin);' if preserve_input_span else 'generator::generate!();'
            source = generated if declarative else invocation
            files = {
                "Cargo.toml": '[workspace]\nmembers=["api"]\nexclude=["macros"]\nresolver="3"\n',
                "api/Cargo.toml": '[package]\nname="api"\nversion="0.1.0"\nedition="2024"\n[dependencies]\ngenerator={path="../macros"}\n[dev-dependencies]\ntraceability={package="traceability-attributes",path=' + json.dumps(str(TOOLS / "tools/traceability-attributes")) + '}\n',
                "api/src/lib.rs": source + '\n#[cfg(test)] mod tests { ' + test + ' }\n',
                "macros/Cargo.toml": '[package]\nname="generator"\nversion="0.1.0"\nedition="2024"\n[lib]\nproc-macro=true\n',
                "macros/src/lib.rs": macro,
            }
            for relative, content in files.items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            trace.execute(["cargo", "+" + trace.toolchain(root), "generate-lockfile"], root)
            if target:
                with self.assertRaisesRegex(trace.InvalidTrace, "ambiguous compiler API identity"):
                    trace.load_traces(root)
            else:
                bundle, _ = trace.load_traces(root)
                occurrences = [api for api in bundle["apis"] if api["identity"]["kind"] == "compilation-occurrence"]
                self.assertTrue(occurrences)
                apis = {api["id"]: api for api in occurrences}
                callers = [call for call in bundle["calls"] if call["caller"] in apis]
                for call in callers:
                    self.assertEqual(len({trace.unit_key(origin) for origin in call["origins"]}), 1,
                                     "ambiguous callers must not merge between compiler units")
                normal = {api["id"] for api in occurrences if not api["identity"]["unit"]["is_test"]}
                test = {api["id"] for api in occurrences if api["identity"]["unit"]["is_test"]}
                self.assertTrue(normal)
                self.assertTrue(test)
                self.assertTrue(normal.isdisjoint(test))

    def test_cfg_macro_cannot_join_a_target_to_another_nominal_receiver(self):
        self.check_macro_identities('''
pub trait Factory { type Product; }
pub struct EntryA;
pub struct EntryB;
pub fn consume_test() {}
pub fn consume_a() {}
pub fn consume_b() {}
#[cfg(test)] const _: () = {
    pub struct Local;
    impl Local { pub fn operation() { consume_test(); } }
};
const _: () = {
    pub struct Local;
    impl Factory for EntryA { type Product = Local; }
    impl Local { pub fn operation() { consume_a(); } }
};
const _: () = {
    pub struct Local;
    impl Factory for EntryB { type Product = Local; }
    impl Local { pub fn operation() { consume_b(); } }
};
pub fn prod_a() { <EntryA as Factory>::Product::operation(); }
pub fn prod_b() { <EntryB as Factory>::Product::operation(); }
''', '<super::EntryA as super::Factory>::Product::operation')

    def test_cfg_macro_equal_sibling_counts_do_not_prove_equivalence(self):
        self.check_macro_identities('''
pub trait Factory { type Product; }
pub struct Entry;
#[cfg(test)] const _: () = { fn different() {} };
const _: () = {
    pub struct Local;
    impl Factory for Entry { type Product = Local; }
    impl Local { pub fn operation() {} }
};
#[cfg(not(test))] const _: () = { fn different() {} };
''', '<super::Entry as super::Factory>::Product::operation')

    def test_ambiguous_private_calls_remain_separate_without_a_target(self):
        self.check_macro_identities('''
#[cfg(test)] const _: () = { fn operation() {} };
const _: () = { fn operation() {} };
const _: () = { fn operation() {} };
''')

    def test_one_macro_sibling_per_build_does_not_prove_equivalence(self):
        self.check_macro_identities('''
#[cfg(not(test))] const _: () = { fn operation() { production_only(); } };
#[cfg(test)] const _: () = { fn operation() { test_only(); } };
pub fn production_only() {}
pub fn test_only() {}
''')

    def test_cfg_macro_const_expression_callers_cannot_be_merged(self):
        self.check_macro_identities('''
pub const fn consume_test() -> usize { 1 }
pub const fn consume_a() -> usize { 1 }
pub const fn consume_b() -> usize { 1 }
pub fn operation() {
    #[cfg(test)] let _: [u8; { consume_test() }] = [0];
    let _: [u8; { consume_a() }] = [0];
    let _: [u8; { consume_b() }] = [0];
}
''')

    def test_macro_input_spans_do_not_make_anonymous_scopes_identifiable(self):
        self.check_macro_identities('''
#[cfg(not(test))] const _: () = { fn operation() { production_only(); } };
#[cfg(test)] const _: () = { fn operation() { test_only(); } };
pub fn production_only() {}
pub fn test_only() {}
''', preserve_input_span=True)

    def test_macro_definition_spans_do_not_make_anonymous_scopes_identifiable(self):
        self.check_macro_identities('''
macro_rules! generate {
    ($callee:path) => { const _: () = { fn operation() { $callee(); } }; };
}
pub fn production_only() {}
pub fn test_only() {}
#[cfg(not(test))] generate!(production_only);
#[cfg(test)] generate!(test_only);
''', declarative=True)

    def test_cfg_macro_identity_split_cannot_silently_drop_a_production_test_target(self):
        self.check_macro_identities('''
pub struct Hidden;
#[cfg(test)] const _: () = { fn padding() {} };
const _: () = { impl Hidden { pub fn hidden() {} } };
''', 'super::Hidden::hidden')

    def test_compiler_inventory_calls_targets_and_warm_cache(self):
        with tempfile.TemporaryDirectory(prefix="compiler-trace-cli-") as directory:
            root = Path(directory)
            (root / "tools").mkdir()
            (root / "tools/traceability").symlink_to(TOOLS / "tools/traceability", target_is_directory=True)
            attributes = TOOLS / "tools/traceability-attributes"
            (root / "Cargo.toml").write_text('[workspace]\nmembers=["api", "adapter"]\nresolver="3"\n')
            for package in ("api", "adapter"):
                (root / package / "src").mkdir(parents=True)
            (root / "api/Cargo.toml").write_text('[package]\nname="api"\nversion="0.1.0"\nedition="2024"\n[dev-dependencies]\nadapter={path="../adapter"}\n')
            (root / "api/src/lib.rs").write_text('''pub trait Port { fn ready(&self) -> bool; }
pub fn calculate() -> bool { true }
include!(concat!(env!("OUT_DIR"), "/generated.rs"));
#[cfg(test)]
mod tests {
    #[test]
    fn setup() { assert!(adapter::execute(&adapter::Store)); }
}
''')
            (root / "api/build.rs").write_text('''fn main() {
    let path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("generated.rs");
    std::fs::write(path, "pub fn generated() -> bool { true }\\n").unwrap();
}
''')
            (root / "adapter/Cargo.toml").write_text('''[package]
name="adapter"
version="0.1.0"
edition="2024"
[dependencies]
api={path="../api"}
[dev-dependencies]
traceability={package="traceability-attributes",path=''' + json.dumps(str(attributes)) + '''}
[features]
extra=[]
[[test]]
name="optional"
path="tests/optional.rs"
required-features=["extra"]
''')
            (root / "adapter/src/lib.rs").write_text('''pub struct Store;
impl api::Port for Store { fn ready(&self) -> bool { api::calculate() && api::generated() } }
pub fn execute(port: &dyn api::Port) -> bool { port.ready() }
#[cfg(test)]
mod tests {
    #[traceability::targets(<dyn api::Port>::ready)]
    #[test]
    fn targeted() { assert!(super::execute(&super::Store)); }
    #[traceability::targets(<super::Store as api::Port>::ready)]
    #[test]
    fn concrete() { assert!(super::execute(&super::Store)); }
    #[test]
    fn untargeted() { assert!(super::execute(&super::Store)); }
}
''')
            for package in ("api", "adapter"):
                (root / package / "src/main.rs").write_text('fn main() { let _ = api::calculate(); }\n')
            (root / "adapter/tests").mkdir()
            (root / "adapter/tests/optional.rs").write_text('compile_error!("disabled target must not compile");\n')
            trace.execute(["cargo", "+" + trace.toolchain(root), "generate-lockfile"], root)
            first, test_artifacts = trace.load_traces(root)
            second, _ = trace.load_traces(root)
            self.assertEqual(first, second, "warm Cargo cache must produce the same fresh compiler snapshot")
            self.assertEqual(len(first["tests"]), 4)
            tests = {test["name"]: test for test in first["tests"]}
            self.assertEqual(tests["tests::untargeted"]["targets"], [])
            apis = {api["id"]: api for api in first["apis"]}
            interface = apis[tests["tests::targeted"]["targets"][0]]
            concrete = apis[tests["tests::concrete"]["targets"][0]]
            self.assertEqual(interface["kind"], "trait-method")
            self.assertEqual(concrete["kind"], "trait-implementation-method")
            self.assertNotEqual(interface["id"], concrete["id"])
            dyn_calls = [call for call in first["calls"] if call["dispatch"] == "dyn"]
            self.assertTrue(dyn_calls)
            self.assertTrue(all(apis[call["callee"]]["kind"] == "trait-method" for call in dyn_calls))
            self.assertTrue(any(apis[call["caller"]]["package"] == "adapter" and apis[call["callee"]]["package"] == "api" for call in first["calls"]))
            production = [call for call in first["calls"] if any("production" in origin["purposes"] and not origin["is_test"] and origin["target_kind"] in ("lib", "bin") for origin in call["origins"])]
            directions = {(apis[call["caller"]]["package"], apis[call["callee"]]["package"]) for call in production}
            self.assertIn(("adapter", "api"), directions)
            self.assertNotIn(("api", "adapter"), directions)
            setup_calls = [call for call in first["calls"] if apis[call["caller"]]["package"] == "api" and apis[call["callee"]]["package"] == "adapter"]
            self.assertTrue(setup_calls, "the real test-only dependency must remain in raw calls")
            self.assertTrue(all(all(origin["is_test"] for origin in call["origins"]) for call in setup_calls))
            shared_calls = [call for call in production if call["caller"] == concrete["id"]]
            self.assertTrue(shared_calls)
            self.assertTrue(all({origin["is_test"] for origin in call["origins"]} == {False, True} for call in shared_calls))
            normal_bins = {unit["package"] for unit in first["units"] if unit["target_kind"] == "bin" and not unit["is_test"]}
            self.assertEqual(normal_bins, {"api", "adapter"}, "all application binaries need normal production compilations")
            self.assertIn("@generated/api/generated.rs", first["generated_sources"])
            self.assertTrue(any(api["path"] == "@generated/api/generated.rs" for api in first["apis"]))
            self.assertEqual(first["excluded_targets"][0]["target_name"], "optional")
            self.assertEqual(first["excluded_targets"][0]["required_features"], ["extra"])
            destination = root / "target/viewer.html"
            write_viewer(root, first, destination)
            self.assertIn('id="traceability-data"', destination.read_text())
            if os.environ.get("TRACEABILITY_TEST_VIEWER"):
                write_viewer(root, first, Path(os.environ["TRACEABILITY_TEST_VIEWER"]))
            trace.run_tests(first["tests"], test_artifacts)

    def test_dev_dependency_features_do_not_create_production_edges(self):
        with tempfile.TemporaryDirectory(prefix="compiler-trace-features-") as directory:
            root = Path(directory)
            (root / "tools").mkdir()
            (root / "tools/traceability").symlink_to(TOOLS / "tools/traceability", target_is_directory=True)
            files = {
                "Cargo.toml": '[workspace]\nmembers=["a", "b"]\nresolver="3"\n',
                "a/Cargo.toml": '[package]\nname="a"\nversion="0.1.0"\nedition="2024"\n[features]\nx=[]\n[dependencies]\nb={path="../b"}\n',
                "a/src/lib.rs": '#[cfg(feature="x")] pub fn feature_only() { b::plain(); }\npub fn plain() {}\n#[cfg(all(test, feature="x"))] mod tests { #[test] fn workspace_feature() { assert!(std::path::Path::new("src/lib.rs").is_file()); } }\n',
                "b/Cargo.toml": '[package]\nname="b"\nversion="0.1.0"\nedition="2024"\n[dev-dependencies]\na={path="../a",features=["x"]}\n',
                "b/src/lib.rs": 'pub fn plain() {}\n#[cfg(test)] mod tests { #[test] fn check() { a::feature_only(); } }\n',
            }
            for path, content in files.items():
                destination = root / path
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_text(content)
            trace.execute(["cargo", "+" + trace.toolchain(root), "generate-lockfile"], root)
            first, test_artifacts = trace.load_traces(root)
            second, _ = trace.load_traces(root)
            self.assertEqual(first, second, "repeated capture must preserve feature and phase provenance")
            normal_a = [unit for unit in first["units"] if unit["package"] == "a" and not unit["is_test"]]
            self.assertEqual({tuple(unit["features"]): unit["purposes"] for unit in normal_a},
                             {(): ["production"], ("x",): ["tests"]})
            normal_b = [unit for unit in first["units"] if unit["package"] == "b" and not unit["is_test"]]
            self.assertEqual(len(normal_b), 1)
            self.assertEqual(normal_b[0]["purposes"], ["production", "tests"],
                             "the normal library cached between Cargo phases must retain both purposes")
            apis = {api["id"]: api for api in first["apis"]}
            feature_calls = [call for call in first["calls"] if apis[call["caller"]]["name"] == "a::feature_only"]
            self.assertTrue(feature_calls)
            self.assertTrue(any(not unit["is_test"] for call in feature_calls for unit in call["origins"]))
            self.assertTrue(all(unit["features"] == ["x"] and unit["purposes"] == ["tests"]
                                for call in feature_calls for unit in call["origins"]))
            production = [call for call in first["calls"] if any("production" in unit["purposes"] and not unit["is_test"]
                          and unit["target_kind"] in ("lib", "bin") for unit in call["origins"])]
            self.assertFalse(any(apis[call["caller"]]["package"] == "a" and apis[call["callee"]]["package"] == "b"
                                 for call in production), "a dev-activated feature must not create a production dependency")
            self.assertNotIn(str(root), json.dumps(first), "runtime artifact paths must stay out of the deterministic report")
            self.assertEqual(len(first["tests"]), 2)
            trace.run_tests(first["tests"], test_artifacts)


if __name__ == "__main__":
    unittest.main()
