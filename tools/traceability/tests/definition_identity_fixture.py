#!/usr/bin/env python3
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

HERE = Path(__file__).resolve().parents[1]
REPO = HERE.parents[1]
TOOLCHAIN = "nightly-2026-11-18"


def run(arguments, *, cwd, env):
    result = subprocess.run(arguments, cwd=cwd, env=env, text=True, capture_output=True)
    if result.returncode:
        lines = [line for line in result.stderr.splitlines() if "compiler API identity" in line]
        diagnostic = lines[0][:500] if lines else result.stderr[-3000:]
        raise RuntimeError(f"{' '.join(map(str, arguments))}\n{diagnostic}")
    return result


def collect(root, cargo, env, driver):
    metadata = run(cargo + ["metadata", "--offline", "--format-version=1"], cwd=root, env=env)
    metadata_path = root / "metadata.json"
    metadata_path.write_text(metadata.stdout)
    wrapped = dict(env, TRACEABILITY_ROOT=str(root), TRACEABILITY_METADATA=str(metadata_path), TRACEABILITY_SHARDS=str(root / "shards"), RUSTC_WORKSPACE_WRAPPER=str(driver))
    run(cargo + ["test", "--no-run", "--workspace", "--offline"], cwd=root, env=wrapped)
    shards = [json.loads(path.read_text()) for path in (root / "shards").glob("*.json")]
    assert len(shards) == 2, [shard["unit"] for shard in shards]
    apis = {}
    for shard in shards:
        assert shard["version"] == 4, shard["version"]
        for category in ("apis", "calls", "references", "tests", "unresolved"):
            for record in shard[category]:
                source = shard["generated_sources"].get(record["path"])
                if source is None:
                    source = (root / record["path"]).read_text()
                assert 0 < record["line"] <= len(source.splitlines()), record
        for api in shard["apis"]:
            if api["identity"]["kind"] == "compilation-occurrence":
                assert api["identity"]["unit"] == shard["unit"], api
            assert all(str(root) not in api[field] and "/home/" not in api[field] and "/tmp/" not in api[field] for field in ("id", "name", "path")), api
            if api["id"] in apis:
                assert apis[api["id"]] == api, (apis[api["id"]], api)
            apis[api["id"]] = api
        for edge in shard["calls"] + shard["references"]:
            assert edge["caller"] in {api["id"] for api in shard["apis"]}, edge
            assert edge["callee"] in {api["id"] for api in shard["apis"]}, edge
        coverage = shard["coverage"]
        assert coverage["calls"] == coverage["workspace_calls"] + coverage["external_calls"] + coverage["unresolved_calls"], coverage
    normal = next(shard for shard in shards if not shard["unit"]["is_test"])
    compiled_test = next(shard for shard in shards if shard["unit"]["is_test"])
    production_target = next(api["id"] for api in normal["apis"] if api["name"] == "definition_api::Hidden::hidden")
    declared = next(test for test in compiled_test["tests"] if test["name"] == "tests::hidden_target")
    assert declared["targets"] == [production_target], {"production_id": production_target, "test_targets": declared["targets"]}
    interface = next(test for test in compiled_test["tests"] if test["name"] == "tests::unused_interface_target")
    unused = [api["id"] for api in normal["apis"] if api["name"] == "definition_api::Unused::unused"]
    assert interface["targets"] == unused, {"declared_interface_targets": len(interface["targets"]), "production_interface_declarations": len(unused)}
    constants = [api for api in apis.values() if "AnonConst#" in api["id"] and "operation" in api["name"]]
    assert len(constants) >= 4 and len({api["id"] for api in constants}) == len(constants), constants
    assert len({(api["path"], api["line"]) for api in constants}) == 1, constants
    constructors = [api for api in apis.values() if api["kind"] == "constructor"]
    assert constructors and all("Ctor#" in api["id"] for api in constructors), constructors
    assert any(api["kind"] == "closure" and api["identity"]["kind"] == "compilation-occurrence" and "@compiler-source/external_traits/" in api["identity"]["reason"] and api["path"] == "api/src/lib.rs" for api in apis.values()), apis
    methods = [api for api in apis.values() if api["kind"] == "trait-implementation-method"]
    assert len(methods) == 2 and len({api["id"] for api in methods}) == 2, methods
    assert all("@compiler-source/" in api["id"] for api in methods), methods
    production_ids = {api["id"] for api in normal["apis"] if api["identity"]["kind"] == "definition"}
    test_ids = {api["id"] for api in compiled_test["apis"]}
    assert production_ids <= test_ids, list(production_ids - test_ids)
    return apis, len(constants)


def observe_ambiguous(root, cargo, env, driver, expected_units=2):
    metadata = run(cargo + ["metadata", "--offline", "--format-version=1"], cwd=root, env=env)
    metadata_path = root / "metadata.json"
    metadata_path.write_text(metadata.stdout)
    wrapped = dict(env, TRACEABILITY_ROOT=str(root), TRACEABILITY_METADATA=str(metadata_path), TRACEABILITY_SHARDS=str(root / "shards"), RUSTC_WORKSPACE_WRAPPER=str(driver))
    result = subprocess.run(cargo + ["test", "--no-run", "--workspace", "--offline"], cwd=root, env=wrapped, text=True, capture_output=True)
    assert result.returncode == 0, result.stderr[-3000:]
    shards = [json.loads(path.read_text()) for path in (root / "shards").glob("*.json")]
    assert len(shards) == expected_units, [shard["unit"] for shard in shards]
    observed = []
    for shard in shards:
        occurrences = [api for api in shard["apis"] if api["identity"]["kind"] == "compilation-occurrence"]
        assert occurrences, shard
        assert all(api["identity"]["unit"] == shard["unit"] for api in occurrences), occurrences
        assert all("ambiguous compiler API identity" in api["identity"]["reason"] for api in occurrences), occurrences
        observed.append({api["id"] for api in occurrences})
    for index, ids in enumerate(observed):
        assert all(ids.isdisjoint(other) for other in observed[index + 1:]), observed


def reject_target(root, cargo, env, driver, target):
    source = root / "api/src/lib.rs"
    source.write_text(source.read_text() + f"\n#[cfg(test)] mod tests {{ #[traceability::targets({target})] #[test] fn target() {{}} }}\n")
    wrapped = dict(env, TRACEABILITY_ROOT=str(root), TRACEABILITY_METADATA=str(root / "metadata.json"), TRACEABILITY_SHARDS=str(root / "target-shards"), RUSTC_WORKSPACE_WRAPPER=str(driver))
    result = subprocess.run(cargo + ["test", "--no-run", "--workspace", "--offline"], cwd=root, env=wrapped, text=True, capture_output=True)
    assert result.returncode, result.stderr
    assert "ambiguous compiler API identity" in result.stderr, result.stderr[-3000:]


def main():
    env = os.environ.copy()
    env.pop("RUSTC_WORKSPACE_WRAPPER", None)
    cargo_name = env.get("TRACEABILITY_CARGO", "cargo")
    cargo = [cargo_name] if env.get("TRACEABILITY_CARGO") else [cargo_name, f"+{TOOLCHAIN}"]
    run(cargo + ["build", "--manifest-path", str(HERE / "Cargo.toml"), "--offline"], cwd=REPO, env=env)
    driver = HERE / "target/debug/traceability-collector"
    with tempfile.TemporaryDirectory(prefix="traceability-definition-identity-") as directory:
        roots = [Path(directory) / "first", Path(directory) / "relocated"]
        files = {
            "Cargo.toml": '[workspace]\nresolver="3"\nmembers=["api"]\nexclude=["macros","external-one","external-two"]\n',
            "api/Cargo.toml": '[package]\nname="definition-api"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nmacros={package="identity-macros",path="../macros"}\nfirst={package="external-traits",path="../external-one"}\nsecond={package="external-traits",path="../external-two"}\n[dev-dependencies]\ntraceability={package="traceability-attributes",path=' + json.dumps(str(REPO / "tools/traceability-attributes")) + '}\n',
            "macros/Cargo.toml": '[package]\nname="identity-macros"\nversion="0.1.0"\nedition="2024"\n[lib]\nproc-macro=true\n',
            "macros/src/lib.rs": '''use proc_macro::TokenStream;
#[proc_macro]
pub fn generate(_: TokenStream) -> TokenStream {
    "pub struct Generated(pub u8); impl Generated { pub fn operation() -> u8 { 1 } }".parse().unwrap()
}
''',
            "api/src/lib.rs": '''macros::generate!();
pub struct One;
pub struct Two;
pub struct Hidden;
pub struct Constants;
impl Constants { pub fn operation() -> [u8; { 1 }] { let value: [u8; { 1 }] = [0; { 1 }]; let _: [u8; { 1 }] = value; value } pub fn opaque() -> impl Fn() { || {} } }
pub trait Unused { fn unused(&self); }
#[cfg(test)]
const _: () = { fn padding() {} };
const _: () = { impl Hidden { pub fn hidden() {} } };
#[cfg(test)]
impl Two { pub fn padding() {} }
impl first::Port for One { fn read(&self) {} }
impl second::Port for Two { fn read(&self) {} }
pub fn external_closure() -> u8 { first::generated_closure!()() }
pub enum Event { Item(u8) }
pub fn construct() -> (Generated, Event) { (Generated(1), Event::Item(2)) }
#[cfg(test)]
mod tests {
    #[test] fn works() { assert_eq!(super::Generated::operation(), 1); assert_eq!(super::Constants::operation(), [0]); }
    #[traceability::targets(super::Hidden::hidden)]
    #[test] fn hidden_target() { super::Hidden::hidden(); }
    #[traceability::targets(<dyn super::Unused>::unused)]
    #[test] fn unused_interface_target() {}
}
''',
        }
        for name, version in [("external-one", "1.0.0"), ("external-two", "2.0.0")]:
            files[f"{name}/Cargo.toml"] = f'[package]\nname="external-traits"\nversion="{version}"\nedition="2024"\n'
            files[f"{name}/src/lib.rs"] = '\n' * 100 + 'pub trait Port { fn read(&self); }\n#[macro_export]\nmacro_rules! generated_closure { () => { || 1u8 }; }\n'
        for relative, content in files.items():
            path = roots[0] / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content)
        shutil.copytree(roots[0], roots[1])
        first, constants = collect(roots[0], cargo, env, driver)
        second, _ = collect(roots[1], cargo, env, driver)
        assert first == second, {"only_first": list(first.keys() - second.keys()), "only_relocated": list(second.keys() - first.keys())}
        ambiguous_cases = {
            "anonymous_literal_expressions": "pub fn operation() -> [u8; { 1 }] { [0; { 1 }] }",
            "unique_private_scope": "const _: () = { fn operation() {} };",
            "cfg_replacement_private_scope": "#[cfg(test)] const _: () = { fn operation() {} }; #[cfg(not(test))] const _: () = { fn operation() {} };",
            "same_name_different_blocks": "pub fn outer() { { fn operation() {} operation(); } { fn operation() {} operation(); } }",
        }
        target_cases = {
            "normalized_fnptr_projection": "pub trait Factory {type Product;} pub struct Entry; pub struct Holder<T>(T); const _: () = {pub enum Local {} impl Factory for Entry {type Product=Local;} }; impl Holder<fn(<Entry as Factory>::Product)> {pub fn operation(){}}",
            "nested_impl_receiver_dependency": "pub trait Factory {type Product;} pub struct Entry; pub struct Inner; pub struct Holder<T>(T); const _: () = {pub enum Hidden {} impl Factory for Inner {type Product=Hidden;} }; impl Holder<<Inner as Factory>::Product> {pub fn declare(){pub enum Local {} impl Factory for Entry {type Product=Local;} }} impl Holder<fn(<Entry as Factory>::Product)> {pub fn operation(){}}",
        }
        ambiguous_cases.update(target_cases)
        for name, generated in ambiguous_cases.items():
            root = Path(directory) / name
            for relative, content in files.items():
                if relative == "macros/src/lib.rs":
                    content = 'use proc_macro::TokenStream; #[proc_macro] pub fn generate(_: TokenStream) -> TokenStream { ' + json.dumps(generated) + '.parse().unwrap() }'
                elif relative == "api/src/lib.rs":
                    content = "macros::generate!();\n"
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            observe_ambiguous(root, cargo, env, driver)
            if name in target_cases:
                receiver = "fn(<super::Entry as super::Factory>::Product)"
                reject_target(root, cargo, env, driver, f"super::Holder::<{receiver}>::operation")
        root = Path(directory) / "external_anonymous_observation"
        external_files = dict(files)
        external_files["Cargo.toml"] = '[workspace]\nresolver="3"\nmembers=["api","observer"]\nexclude=["macros","external-one","external-two"]\n'
        external_files["api/src/lib.rs"] = "macros::generate!();\n"
        generated = "pub trait Factory {type Product;} pub struct Entry; const _: () = {pub enum Local {} impl Factory for Entry {type Product=Local;} impl Local {pub fn operation(){}} };"
        external_files["macros/src/lib.rs"] = 'use proc_macro::TokenStream; #[proc_macro] pub fn generate(_: TokenStream) -> TokenStream { ' + json.dumps(generated) + '.parse().unwrap() }'
        external_files["observer/Cargo.toml"] = '[package]\nname="observer"\nversion="0.1.0"\nedition="2024"\n[dependencies]\napi={package="definition-api",path="../api"}\n'
        external_files["observer/src/lib.rs"] = "pub fn observe() { <api::Entry as api::Factory>::Product::operation(); }\n"
        for relative, content in external_files.items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content)
        observe_ambiguous(root, cargo, env, driver, expected_units=4)
        print(json.dumps({"anonymous_constants": constants, "relocated_api_count": len(first), "external_trait_versions": 2, "ambiguous_occurrence_cases": list(ambiguous_cases), "rejected_ambiguous_targets": list(target_cases), "external_anonymous_observer_units": 4, "observations": ["distinct-span anonymous constants", "proc macro direct implementation", "opaque return type", "constructor namespace", "normal/test stability", "typed target inside cfg-shifting anonymous const", "unused trait declaration target", "external source hashes", "external macro span origins", "distinct external crate versions", "checkout relocation"]}, indent=2))


if __name__ == "__main__":
    main()
