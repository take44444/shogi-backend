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
        raise RuntimeError(f"{' '.join(map(str, arguments))}\n{result.stderr[-3000:]}")
    return result


def main():
    env = os.environ.copy()
    env.pop("RUSTC_WORKSPACE_WRAPPER", None)
    cargo_name = env.get("TRACEABILITY_CARGO", "cargo")
    cargo = [cargo_name] if env.get("TRACEABILITY_CARGO") else [cargo_name, f"+{TOOLCHAIN}"]
    run(cargo + ["build", "--manifest-path", str(HERE / "Cargo.toml"), "--offline"], cwd=REPO, env=env)
    driver = HERE / "target/debug/traceability-collector"
    with tempfile.TemporaryDirectory(prefix="traceability-external-identity-") as directory:
        root = Path(directory)
        files = {
            "Cargo.toml": '[workspace]\nresolver="3"\nmembers=["api","activator"]\nexclude=["external-one","external-two"]\n',
            "api/Cargo.toml": '[package]\nname="external-identity-api"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nfirst={package="external-traits",path="../external-one"}\nsecond={package="external-traits",path="../external-two"}\n[dev-dependencies]\ntraceability={package="traceability-attributes",path=' + json.dumps(str(REPO / "tools/traceability-attributes")) + '}\n',
            "api/src/lib.rs": '''pub struct Store;
impl first::Port for Store { fn read(&self) {} }
impl second::Port for Store { fn read(&self) {} }
#[cfg(test)]
mod tests {
    #[traceability::targets(<super::Store as first::Port>::read)]
    #[test] fn first_target() {}
    #[traceability::targets(<super::Store as second::Port>::read)]
    #[test] fn second_target() {}
}
''',
            "activator/Cargo.toml": '[package]\nname="external-identity-activator"\nversion="0.1.0"\nedition="2024"\n[dependencies]\napi={package="external-identity-api",path="../api"}\n[dev-dependencies]\nfirst={package="external-traits",path="../external-one",features=["extra"]}\n',
            "activator/src/lib.rs": 'pub fn store() -> api::Store { api::Store }\n',
        }
        for name, version in [("external-one", "1.0.0"), ("external-two", "2.0.0")]:
            files[f"{name}/Cargo.toml"] = f'[package]\nname="external-traits"\nversion="{version}"\nedition="2024"\n[features]\nextra=[]\n'
            files[f"{name}/src/lib.rs"] = 'pub trait Port { fn read(&self); }\n#[cfg(feature="extra")] pub fn extra() {}\n'
        for relative, content in files.items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content)
        metadata = run(cargo + ["metadata", "--offline", "--format-version=1"], cwd=root, env=env)
        metadata_path = root / "metadata.json"
        metadata_path.write_text(metadata.stdout)
        wrapped = dict(env, TRACEABILITY_ROOT=str(root), TRACEABILITY_METADATA=str(metadata_path), TRACEABILITY_SHARDS=str(root / "shards"), RUSTC_WORKSPACE_WRAPPER=str(driver))
        builds = [
            run(cargo + ["build", "-p", "external-identity-api", "--offline", "--message-format=json"], cwd=root, env=wrapped),
            run(cargo + ["test", "--no-run", "--workspace", "--offline", "--message-format=json"], cwd=root, env=wrapped),
        ]
        external_features = []
        for build in builds:
            external_features.append({tuple(artifact["features"]) for line in build.stdout.splitlines() if (artifact := json.loads(line)).get("reason") == "compiler-artifact" and "external-one" in artifact["package_id"]})
        assert external_features == [{()}, {("extra",)}], external_features
        shards = [json.loads(path.read_text()) for path in (root / "shards").glob("*.json")]
        api_units = [shard for shard in shards if shard["unit"]["package"] == "external-identity-api"]
        normal = [shard for shard in api_units if not shard["unit"]["is_test"]]
        assert len(normal) == 2 and normal[0]["unit"] == normal[1]["unit"], [shard["unit"] for shard in api_units]
        implementations = [{api["id"] for api in shard["apis"] if api["kind"] == "trait-implementation-method"} for shard in api_units]
        assert all(len(ids) == 2 for ids in implementations), [len(ids) for ids in implementations]
        assert all(ids == implementations[0] for ids in implementations), {"stable_implementation_counts": [len(ids & implementations[0]) for ids in implementations], "expected_each": 2}
        compiled_test = next(shard for shard in api_units if shard["unit"]["is_test"])
        targets = {target for test in compiled_test["tests"] for target in test["targets"]}
        assert targets == implementations[0], {"matching_exact_targets": len(targets & implementations[0]), "expected": 2}
        assert all("/tmp/" not in identity and "/home/" not in identity for identity in targets)
        print(json.dumps({"external_features": [[list(features) for features in build] for build in external_features], "stable_library_units": len(api_units), "exact_targets": len(targets), "distinct_external_versions": 2}, indent=2))


if __name__ == "__main__":
    main()
