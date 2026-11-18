#!/usr/bin/env python3
"""Exercise compiler identity, call extraction, and explicit test targets in two crates."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

HERE = Path(__file__).resolve().parents[1]
REPO = HERE.parents[1]
TOOLCHAIN = "nightly-2026-11-18"


def run(arguments, *, cwd, env):
    result = subprocess.run(arguments, cwd=cwd, env=env, text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError(f"{' '.join(map(str, arguments))}\n{result.stdout}\n{result.stderr}")
    return result


def main():
    env = os.environ.copy()
    env.pop("RUSTC_WORKSPACE_WRAPPER", None)
    cargo = env.get("TRACEABILITY_CARGO", "cargo")
    cargo_args = [cargo] if env.get("TRACEABILITY_CARGO") else [cargo, f"+{TOOLCHAIN}"]
    run(cargo_args + ["build", "--manifest-path", str(HERE / "Cargo.toml"), "--offline"], cwd=REPO, env=env)
    driver = HERE / "target/debug/traceability-collector"
    with tempfile.TemporaryDirectory(prefix="traceability-fixture-") as directory:
        root = Path(directory)
        files = {
            "Cargo.toml": '[workspace]\nresolver="3"\nmembers=["api", "consumer"]\n',
            "api/Cargo.toml": '[package]\nname="boundary-api"\nversion="0.1.0"\nedition="2024"\n[dev-dependencies]\ntraceability={package="traceability-attributes",path=' + json.dumps(str(REPO / "tools/traceability-attributes")) + '}\n',
            "api/build.rs": 'fn main() { let out=std::env::var("OUT_DIR").unwrap(); std::fs::write(std::path::Path::new(&out).join("generated.rs"), "pub fn generated() -> u32 { calculate() }\\n").unwrap(); }',
            "api/src/lib.rs": '''
include!(concat!(env!("OUT_DIR"), "/generated.rs"));
pub trait Port { fn ready(&self) -> u32; }
pub struct Store;
impl Store {
    pub fn new() -> Self { Self }
    pub fn read(&self) -> u32 { 7 }
}
impl Port for Store { fn ready(&self) -> u32 { self.read() } }
pub fn calculate() -> u32 { 5 }
pub const CONSTANT:u32=3;
fn private() {}
#[macro_export]
macro_rules! call_read { ($s:expr) => { $s.read() }; }
#[cfg(test)]
mod tests {
    #[traceability::targets(super::calculate)]
    #[test]
    fn intended_free_api() { assert_eq!(super::Store::new().read(), 7); }
    #[traceability::targets(<super::Store as super::Port>::ready)]
    #[test]
    fn concrete_implementation() { assert_eq!(1, 1); }
    #[test]
    fn unannotated() { assert_eq!(super::calculate(), 5); }
}
''',
            "consumer/Cargo.toml": '[package]\nname="boundary-consumer"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nrenamed={package="boundary-api",path="../api"}\n[dev-dependencies]\ntraceability={package="traceability-attributes",path=' + json.dumps(str(REPO / "tools/traceability-attributes")) + '}\n',
            "consumer/src/lib.rs": '''
use renamed::{Port, Store as Alias};
pub fn aliases() -> u32 { let s=Alias::new(); renamed::call_read!(s)+renamed::calculate()+renamed::generated() }
pub fn generic<T:Port>(port:&T) -> u32 { port.ready() }
pub fn dynamic(port:&dyn Port) -> u32 { port.ready() }
pub fn concrete(port:&Alias) -> u32 { port.ready() }
pub fn through_closure() -> u32 { let f=||renamed::calculate(); f() }
pub async fn through_async() -> u32 { renamed::calculate() }
pub fn indirect(f:fn()->u32) -> u32 { f() }
''',
            "consumer/tests/boundaries.rs": '''
#[traceability::targets(<dyn renamed::Port>::ready, renamed::Store::read)]
#[test]
fn declared_boundary() { assert_eq!(renamed::calculate(), 5); }
#[traceability::targets(renamed::Store::new)]
#[test]
fn a_second_root() { assert_eq!(1, 1); }
''',
        }
        for relative, content in files.items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content)
        metadata = run(cargo_args + ["metadata", "--offline", "--no-deps", "--format-version=1"], cwd=root, env=env)
        metadata_path = root / "metadata.json"
        metadata_path.write_text(metadata.stdout)
        env.update(TRACEABILITY_ROOT=str(root), TRACEABILITY_METADATA=str(metadata_path), TRACEABILITY_SHARDS=str(root / "shards"), RUSTC_WORKSPACE_WRAPPER=str(driver))
        build = run(cargo_args + ["test", "--no-run", "--workspace", "--offline", "--message-format=json"], cwd=root, env=env)
        shards = [json.loads(path.read_text()) for path in (root / "shards").glob("*.json")]
        assert len(shards) == 6, [(shard["unit"], path) for shard in shards for path in []]
        apis = {}
        for shard in shards:
            for api in shard["apis"]:
                if api["id"] in apis:
                    assert apis[api["id"]] == api, (apis[api["id"]], api)
                apis[api["id"]] = api
        calls = [call for shard in shards for call in shard["calls"]]
        boundary = [call for call in calls if call["caller_package"] == "boundary-consumer" and apis[call["callee"]]["package"] == "boundary-api"]
        names = {apis[call["callee"]]["name"].split("::")[-1] for call in boundary}
        assert {"new", "read", "calculate", "ready"} <= names, names
        dispatches = {call["dispatch"] for call in boundary if apis[call["callee"]]["name"].endswith("::ready")}
        assert {"generic", "dyn", "trait"} <= dispatches, dispatches
        assert all(apis[call["callee"]]["kind"] == "trait-method" for call in boundary if apis[call["callee"]]["name"].endswith("::ready"))
        assert any(call.get("expansion") for call in boundary if apis[call["callee"]]["name"].endswith("::read")), boundary
        assert any("through_async" in apis[call["caller"]]["name"] and apis[call["callee"]]["name"].endswith("::calculate") for call in boundary), [(apis[call["caller"]]["name"],apis[call["callee"]]["name"]) for call in boundary]
        assert any("through_closure" in apis[call["caller"]]["name"] and apis[call["callee"]]["name"].endswith("::calculate") for call in boundary)
        tests = {test["name"]: test for shard in shards for test in shard["tests"]}
        assert set(tests) == {"tests::intended_free_api", "tests::concrete_implementation", "tests::unannotated", "declared_boundary", "a_second_root"}, tests
        assert [apis[target]["name"] for target in tests["tests::intended_free_api"]["targets"]] == ["boundary_api::calculate"], [apis[target] for target in tests["tests::intended_free_api"]["targets"]]
        assert tests["tests::unannotated"]["targets"] == []
        concrete_targets = [apis[target] for target in tests["tests::concrete_implementation"]["targets"]]
        assert len(concrete_targets) == 1 and concrete_targets[0]["kind"] == "trait-implementation-method", concrete_targets
        dynamic_targets = [apis[target] for target in tests["declared_boundary"]["targets"]]
        assert {api["kind"] for api in dynamic_targets} == {"trait-method", "inherent-method"}, dynamic_targets
        library_units = [shard for shard in shards if shard["unit"]["package"] == "boundary-api" and shard["unit"]["target_kind"] == "lib"]
        generated = {key: value for shard in shards for key, value in shard["generated_sources"].items()}
        assert generated == {"@generated/boundary-api/generated.rs": "pub fn generated() -> u32 { calculate() }\n"}, generated
        assert any(api["path"] == "@generated/boundary-api/generated.rs" for api in apis.values())
        ids = [{api["id"] for api in shard["apis"] if api["name"] == "boundary_api::calculate"} for shard in library_units]
        assert len(ids) == 2 and ids[0] == ids[1] and len(ids[0]) == 1, ids
        unresolved = [item for shard in shards for item in shard["unresolved"]]
        assert len(unresolved) == 2 and all("function pointer" in item["reason"] for item in unresolved), unresolved
        for shard in shards:
            coverage = shard["coverage"]
            assert coverage["calls"] == coverage["workspace_calls"] + coverage["external_calls"] + coverage["unresolved_calls"], coverage
        metadata_value = json.loads(metadata.stdout)
        packages = {package["id"]: package["name"] for package in metadata_value["packages"]}
        for line in build.stdout.splitlines():
            artifact = json.loads(line)
            if artifact.get("reason") != "compiler-artifact" or not artifact["profile"]["test"]:
                continue
            listed = run([artifact["executable"], "--list", "--format", "terse"], cwd=root, env=env)
            actual = {line.removesuffix(": test") for line in listed.stdout.splitlines() if line.endswith(": test")}
            target = artifact["target"]
            parsed = {test["name"] for test in tests.values() if test["package"] == packages[artifact["package_id"]] and test["target_kind"] == target["kind"][0] and test["target_name"] == target["name"]}
            assert parsed == actual, (parsed, actual)
        invalid_cases = [
            ('#[traceability::targets(renamed::calculate, renamed::CONSTANT)]\n#[test]\nfn invalid() {}', "must resolve to a Rust function or method item"),
            ('#[traceability::targets(renamed::calculate)]\nfn not_test() {}', "no compiler test named not_test"),
            ('#[traceability::targets(renamed::missing)]\n#[test]\nfn invalid() {}', "cannot find value"),
            ('#[traceability::targets(renamed::private)]\n#[test]\nfn invalid() {}', "is private"),
        ]
        for source, diagnostic in invalid_cases:
            (root / "consumer/tests/boundaries.rs").write_text(source)
            result = subprocess.run(cargo_args + ["test", "--no-run", "-p", "boundary-consumer", "--test", "boundaries", "--offline"], cwd=root, env=env, text=True, capture_output=True)
            assert result.returncode != 0 and diagnostic in result.stderr, (source, result.stdout, result.stderr)
        print(json.dumps({"negative_cases": len(invalid_cases), "units": len(shards), "tests": len(tests), "boundary_calls": len(boundary), "unresolved_function_pointers": len(unresolved), "observations": ["aliases", "macro provenance", "generic trait", "dyn trait", "concrete trait normalization", "closure", "async", "explicit targets only", "exact implementation target", "canonical library IDs"]}, indent=2))


if __name__ == "__main__":
    main()
