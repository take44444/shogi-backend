#!/usr/bin/env python3
"""Run the CLI collection pipeline against real Rust compiler units twice."""
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
            first = trace.load_traces(root)
            second = trace.load_traces(root)
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
            production = [call for call in first["calls"] if any(not origin["is_test"] and origin["target_kind"] in ("lib", "bin") for origin in call["origins"])]
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
            trace.run_tests(first["tests"], root, first["configuration"]["toolchain"])


if __name__ == "__main__":
    unittest.main()
