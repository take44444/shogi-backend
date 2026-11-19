#!/usr/bin/env python3
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
    attributes = json.dumps(str(REPO / "tools/traceability-attributes"))
    with tempfile.TemporaryDirectory(prefix="traceability-method-identity-") as directory:
        root = Path(directory)
        files = {
            "Cargo.toml": '[workspace]\nresolver="3"\nmembers=["api", "consumer"]\n',
            "api/Cargo.toml": '[package]\nname="identity-api"\nversion="0.1.0"\nedition="2024"\n[dev-dependencies]\ntraceability={package="traceability-attributes",path=' + attributes + '}\n',
            "api/src/lib.rs": '''
pub trait One { fn read(&self); }
pub trait Two { fn read(&self); }
pub trait Select<T> { fn read(&self); }
pub trait Value { fn read(&self); }
pub struct Store;
pub struct Container<T>(pub T);
pub struct Left;
pub struct Right;
pub struct Padding;
pub fn one() {}
pub fn two() {}
pub fn left() {}
pub fn right() {}
pub fn value_left() {}
pub fn value_right() {}
#[cfg(test)]
impl Padding { pub fn only_in_tests() {} }
macro_rules! implement { ($($tr:path => $call:ident),*) => {
    $(impl $tr for Store { fn read(&self) { $call(); } })*
}; }
implement!(One => one, Two => two, Select<Left> => left, Select<Right> => right);
macro_rules! values { ($($arg:ty => $call:ident),*) => {
    $(impl Value for Container<$arg> { fn read(&self) { $call(); } })*
}; }
values!(Left=>value_left, Right=>value_right);
#[cfg(test)]
mod tests {
    use super::{Container as Boxed, Left as L, One as First, Right as R, Select as Pick, Store as Alias, Two as Second, Value};
    #[traceability::targets(<Alias as First>::read)]
    #[test]
    fn one_target() {}
    #[traceability::targets(<Alias as Second>::read)]
    #[test]
    fn two_target() {}
    #[traceability::targets(<Alias as Pick<L>>::read)]
    #[test]
    fn left_target() {}
    #[traceability::targets(<Alias as Pick<R>>::read)]
    #[test]
    fn right_target() {}
    #[traceability::targets(<Boxed<L> as Value>::read)]
    #[test]
    fn value_left_target() {}
    #[traceability::targets(<Boxed<R> as Value>::read)]
    #[test]
    fn value_right_target() {}
}
''',
            "consumer/Cargo.toml": '[package]\nname="identity-consumer"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nrenamed={package="identity-api",path="../api"}\n[dev-dependencies]\ntraceability={package="traceability-attributes",path=' + attributes + '}\n',
            "consumer/src/lib.rs": '''
use renamed::{One, Store as Alias, Two};
pub fn call_interfaces(store: &Alias) { One::read(store); Two::read(store); }
''',
            "consumer/tests/targets.rs": '''
use renamed::{Container as Boxed, Left as L, One as First, Right as R, Select as Pick, Store as Alias, Two as Second, Value};
#[traceability::targets(<Alias as First>::read)]
#[test]
fn one_target() {}
#[traceability::targets(<Alias as Second>::read)]
#[test]
fn two_target() {}
#[traceability::targets(<Alias as Pick<L>>::read)]
#[test]
fn left_target() {}
#[traceability::targets(<Alias as Pick<R>>::read)]
#[test]
fn right_target() {}
#[traceability::targets(<Boxed<L> as Value>::read)]
#[test]
fn value_left_target() {}
#[traceability::targets(<Boxed<R> as Value>::read)]
#[test]
fn value_right_target() {}
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
        run(cargo_args + ["test", "--no-run", "--workspace", "--offline"], cwd=root, env=env)
        shards = [json.loads(path.read_text()) for path in (root / "shards").glob("*.json")]
        library_units = [shard for shard in shards if shard["unit"]["package"] == "identity-api" and shard["unit"]["target_kind"] == "lib"]
        assert len(library_units) == 2 and {shard["unit"]["is_test"] for shard in library_units} == {False, True}, [shard["unit"] for shard in shards]
        apis = {}
        for shard in shards:
            for api in shard["apis"]:
                if api["id"] in apis:
                    assert apis[api["id"]] == api, (apis[api["id"]], api)
                apis[api["id"]] = api
        implementations = {api["id"]: api for api in apis.values() if api["kind"] == "trait-implementation-method"}
        assert len(implementations) == 6, {"expected_implementation_apis": 6, "actual_implementation_apis": len(implementations), "apis": list(implementations.values())}
        ids = [{api["id"] for api in shard["apis"] if api["kind"] == "trait-implementation-method"} for shard in library_units]
        assert ids[0] == ids[1] == set(implementations), ids
        targets = {}
        for shard in shards:
            for test in shard["tests"]:
                key = test["name"].removeprefix("tests::").removesuffix("_target")
                assert len(test["targets"]) == 1, test
                target = test["targets"][0]
                assert target in implementations, (test, apis[target])
                if key in targets:
                    assert targets[key] == target, (key, targets[key], target)
                targets[key] = target
        assert set(targets) == {"one", "two", "left", "right", "value_left", "value_right"}, targets
        assert len(set(targets.values())) == 6, targets
        body_edges = []
        for shard in library_units:
            for name, target in targets.items():
                callees = [apis[call["callee"]]["name"] for call in shard["calls"] if call["caller"] == target]
                assert callees == [f"identity_api::{name}"], (name, target, callees)
                body_edges.extend(callees)
        interface_calls = [call for shard in shards for call in shard["calls"] if apis[call["caller"]]["name"] == "identity_consumer::call_interfaces"]
        assert {apis[call["callee"]]["name"] for call in interface_calls} == {"identity_api::One::read", "identity_api::Two::read"}, interface_calls
        assert all(apis[call["callee"]]["kind"] == "trait-method" for call in interface_calls), interface_calls
        print(json.dumps({"implementation_apis": len(implementations), "stable_library_units": len(library_units), "exact_typed_targets": sum(len(shard["tests"]) for shard in shards), "body_edges": len(body_edges), "observations": ["same macro token spans", "distinct trait definitions", "trait generic arguments", "self generic arguments", "cfg(test) preceding implementation", "dependency and import aliases", "interface call normalization"]}, indent=2))


if __name__ == "__main__":
    main()
