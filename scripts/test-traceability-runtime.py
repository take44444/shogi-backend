#!/usr/bin/env python3
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("traceability", Path(__file__).with_name("traceability.py"))
trace = importlib.util.module_from_spec(spec)
spec.loader.exec_module(trace)


@unittest.skipUnless(sys.platform == "linux", "fixture builds a Linux shared library")
class CargoRuntimeTests(unittest.TestCase):
    def test_captured_artifacts_keep_cargo_runtime_and_workspace_features(self):
        with tempfile.TemporaryDirectory(prefix="traceability-runtime-") as directory:
            root = Path(directory)
            files = {
                "Cargo.toml": '[workspace]\nmembers=["a", "b"]\nresolver="3"\n',
                ".cargo/config.toml": '[build]\nrustflags=["-C", "prefer-dynamic"]\n[env]\nTRACEABILITY_CONFIG_VALUE={value="configured",force=true}\n',
                "a/Cargo.toml": '[package]\nname="a"\nversion="1.2.3-preview"\nedition="2024"\nauthors=["Fixture Author"]\ndescription="Runtime fixture"\n[features]\nx=[]\n',
                "a/build.rs": '''fn main() {
    let out = std::env::var("OUT_DIR").unwrap();
    let source = std::path::Path::new(&out).join("runtime.c");
    std::fs::write(&source, "int runtime_value(void) { return 7; }\\n").unwrap();
    assert!(std::process::Command::new("cc").args(["-shared", "-fPIC"])
        .arg(&source).arg("-o").arg(std::path::Path::new(&out).join("libfixture_runtime.so"))
        .status().unwrap().success());
    println!("cargo::rustc-link-search=native={out}");
    println!("cargo::rustc-link-lib=dylib=fixture_runtime");
    println!("cargo::rustc-env=TRACEABILITY_BUILD_VALUE=emitted");
}''',
                "a/src/lib.rs": '''pub fn api() {}
#[cfg(all(test, feature="x"))]
mod tests {
    unsafe extern "C" { fn runtime_value() -> i32; }
    #[test]
    fn runtime_environment() {
        assert_eq!(unsafe { runtime_value() }, 7);
        assert!(std::path::Path::new("src/lib.rs").is_file());
        for (name, expected) in [
            ("CARGO_MANIFEST_DIR", env!("CARGO_MANIFEST_DIR")),
            ("CARGO_MANIFEST_PATH", env!("CARGO_MANIFEST_PATH")),
            ("CARGO_PKG_NAME", "a"),
            ("CARGO_PKG_VERSION", "1.2.3-preview"),
            ("CARGO_PKG_VERSION_MAJOR", "1"),
            ("CARGO_PKG_VERSION_MINOR", "2"),
            ("CARGO_PKG_VERSION_PATCH", "3"),
            ("CARGO_PKG_VERSION_PRE", "preview"),
            ("CARGO_PKG_AUTHORS", "Fixture Author"),
            ("CARGO_PKG_DESCRIPTION", "Runtime fixture"),
            ("TRACEABILITY_BUILD_VALUE", "emitted"),
            ("TRACEABILITY_CONFIG_VALUE", "configured"),
        ] { assert_eq!(std::env::var(name).unwrap(), expected, "{name}"); }
        for key in ["CARGO", "CARGO_MANIFEST_DIR", "CARGO_MANIFEST_PATH", "CARGO_PKG_NAME",
                    "CARGO_PKG_VERSION", "CARGO_PKG_VERSION_MAJOR", "CARGO_PKG_VERSION_MINOR",
                    "CARGO_PKG_VERSION_PATCH", "CARGO_PKG_VERSION_PRE", "CARGO_PKG_AUTHORS",
                    "CARGO_PKG_DESCRIPTION", "CARGO_PKG_HOMEPAGE", "CARGO_PKG_REPOSITORY",
                    "CARGO_PKG_LICENSE", "CARGO_PKG_LICENSE_FILE", "CARGO_PKG_RUST_VERSION",
                    "CARGO_PKG_README", "OUT_DIR", "LD_LIBRARY_PATH"] {
            println!("RUNTIME_SAFE {key}={}", std::env::var(key).unwrap_or_default());
        }
    }
}''',
                "b/Cargo.toml": '[package]\nname="b"\nversion="0.1.0"\nedition="2024"\n[dependencies]\na={path="../a",features=["x"]}\n',
                "b/src/lib.rs": 'pub fn api() { a::api() }\n',
            }
            for name, content in files.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            channel = trace.toolchain(Path(__file__).resolve().parent.parent)
            cargo = ["cargo", "+" + channel]
            metadata = json.loads(trace.execute(cargo + ["metadata", "--format-version=1"], root))
            command = ["test", "--workspace", "--all-targets", "--no-run", "--locked", "--message-format=json"]
            output = trace.execute(cargo + command, root)
            artifacts = trace.cargo_artifacts([("tests", output)], metadata)
            test = {"package": "a", "target_kind": "lib", "target_name": "a", "name": "tests::runtime_environment", "targets": []}
            bundle = {"tests": [test], "components": [{"id": "a", "path": "a"}, {"id": "b", "path": "b"}]}
            before = trace.canonical(bundle)
            captures = root / "captures"
            captures.mkdir()
            runtimes = trace.capture_test_runtimes(cargo, command, root, os.environ.copy(), artifacts, output, captures)
            test_artifacts = trace.check_inventory(bundle, artifacts, root, runtimes)
            trace.run_tests(bundle["tests"], test_artifacts)
            baseline = trace.execute(cargo + [arg for arg in command if arg != "--no-run"] + ["--", "--nocapture"], root)
            runtime = test_artifacts[trace.target_key(test)]
            direct = trace.execute([runtime["executable"], test["name"], "--exact", "--nocapture"],
                                   runtime["directory"], env=runtime["environment"])
            def safe_values(output):
                return dict(line.removeprefix("RUNTIME_SAFE ").split("=", 1) for line in output.splitlines()
                            if line.startswith("RUNTIME_SAFE "))
            self.assertEqual(len(safe_values(baseline)), 19)
            self.assertEqual(safe_values(baseline), safe_values(direct))
            self.assertEqual(trace.canonical(bundle), before)
            self.assertNotIn(str(root), trace.canonical(bundle))
            self.assertEqual(set(runtimes), {artifact["executable"] for artifact in artifacts if artifact["is_test"] and artifact["executable"]})
            self.assertTrue(all("TRACEABILITY_BUILD_VALUE" not in runtime["environment"] for runtime in runtimes.values() if runtime["directory"] == str(root / "b")))


if __name__ == "__main__":
    unittest.main()
