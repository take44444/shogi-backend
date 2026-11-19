#!/usr/bin/env python3
"""Collect compiler-derived Rust calls and explicitly targeted tests."""
import argparse
from collections import Counter
import json
import os
from pathlib import Path, PurePosixPath
import re
import shlex
import subprocess
import sys
import tempfile
import tomllib


class InvalidTrace(Exception):
    pass


def require(condition, message):
    if not condition:
        raise InvalidTrace(message)


def no_duplicate_keys(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON field: {key}")
        result[key] = value
    return result


def read_json(path):
    return json.loads(Path(path).read_text(encoding="utf-8"), object_pairs_hook=no_duplicate_keys)


def text(value):
    require(isinstance(value, str) and bool(value.strip()) and "\n" not in value and "\r" not in value,
            "expected nonempty single-line text")
    return value


def fields(value, required):
    require(isinstance(value, dict) and set(required) <= value.keys(), f"expected object fields {required}")


def repository_file(root, path):
    relative = PurePosixPath(text(path))
    require(not relative.is_absolute() and ".." not in relative.parts and str(relative) == path,
            f"source path must be repository-relative: {path}")
    resolved = (Path(root).resolve() / path).resolve()
    require(resolved.is_relative_to(Path(root).resolve()) and resolved.is_file(),
            f"source file missing or outside repository: {path}")
    return resolved


def source_location(root, record, generated_sources=None):
    fields(record, ("path", "line"))
    if record["path"].startswith("@generated/"):
        require(record["path"] in (generated_sources or {}), f"missing generated source: {record['path']}")
        content = generated_sources[record["path"]]
    else:
        content = repository_file(root, record["path"]).read_text(encoding="utf-8")
    require(type(record["line"]) is int and 0 < record["line"] <= len(content.splitlines()),
            f"invalid compiler source location: {record['path']}:{record['line']}")


def target_key(value):
    return value["package"], value["target_kind"], value["target_name"]


def test_key(value):
    return (*target_key(value), value["name"])


def unit_key(value):
    return (*target_key(value), value["is_test"], tuple(value["features"]))


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def sorted_records(records):
    return sorted(records, key=canonical)


def merge_fact(records, fact, unit):
    record = records.setdefault(canonical(fact), {**fact, "origins": []})
    if unit not in record["origins"]:
        record["origins"] = sorted_records([*record["origins"], unit])


def merge_shards(root, shards, components):
    """Merge compiler units without inferring test intent or implementation relations."""
    shards = list(shards)
    apis, tests, units, calls, references, unresolved, coverage = {}, {}, {}, {}, {}, {}, Counter()
    packages = {component["id"] for component in components}
    generated_sources = {}
    for shard in shards:
        require(isinstance(shard, dict), "compiler shard must be an object")
        sources = shard.get("generated_sources", {})
        require(isinstance(sources, dict), "generated_sources must be an object")
        for path, content in sources.items():
            relative = PurePosixPath(text(path))
            require(len(relative.parts) >= 3 and relative.parts[0] == "@generated"
                    and relative.parts[1] in packages and ".." not in relative.parts and str(relative) == path,
                    f"invalid generated source key: {path}")
            local = Path(root) / path
            require(not local.exists() and not local.is_symlink(), f"generated source key collides with repository file: {path}")
            require(isinstance(content, str), f"generated source contents must be text: {path}")
            require(path not in generated_sources or generated_sources[path] == content,
                    f"conflicting generated source contents: {path}")
            generated_sources[path] = content
    seen = set()
    for shard in shards:
        fields(shard, ("version", "unit", "apis", "calls", "tests", "unresolved", "coverage"))
        require(type(shard["version"]) is int and shard["version"] == 3, "compiler shard version must be 3")
        signature = canonical(shard)
        if signature in seen:
            continue
        seen.add(signature)
        unit = shard["unit"]
        fields(unit, ("package", "crate_name", "target_kind", "target_name", "is_test", "features", "purposes", "source"))
        require(unit["package"] in packages, f"unknown compiler unit package: {unit['package']}")
        require(type(unit["is_test"]) is bool, "unit is_test must be boolean")
        require(isinstance(unit["features"], list) and all(isinstance(feature, str) and feature for feature in unit["features"]),
                "unit features must be an array of nonempty names")
        require(unit["features"] == sorted(set(unit["features"])), "unit features must be sorted and unique")
        require(isinstance(unit["purposes"], list) and all(purpose in ("production", "tests") for purpose in unit["purposes"]),
                "unit purposes must identify Cargo compilation phases")
        require(unit["purposes"] == sorted(set(unit["purposes"])), "unit purposes must be sorted and unique")
        require(unit["purposes"] or unit["target_kind"] == "custom-build", "compiler unit has no Cargo compilation purpose")
        for key in ("crate_name", "target_kind", "target_name"):
            text(unit[key])
        repository_file(root, unit["source"])
        key = unit_key(unit)
        require(key not in units or units[key] == unit, f"conflicting compiler unit: {key}")
        require(key not in units, f"multiple different shards for compiler unit: {key}")
        units[key] = unit
        for category in ("apis", "calls", "tests", "unresolved"):
            require(isinstance(shard[category], list), f"shard {category} must be an array")
        for api in shard["apis"]:
            fields(api, ("id", "package", "name", "kind", "path", "line"))
            identity = text(api["id"])
            require(api["package"] in packages, f"unknown API package: {api['package']}")
            text(api["name"])
            text(api["kind"])
            source_location(root, api, generated_sources)
            require(identity not in apis or apis[identity] == api, f"conflicting API definition: {identity}")
            apis[identity] = api
        for category, records in (("calls", calls), ("references", references)):
            require(isinstance(shard.get(category, []), list), f"shard {category} must be an array")
            for call in shard.get(category, []):
                fields(call, ("caller", "caller_package", "callee", "path", "line", "dispatch"))
                text(call["caller"])
                text(call["callee"])
                text(call["dispatch"])
                require(call["caller_package"] == unit["package"], "call package differs from compiler unit")
                source_location(root, call, generated_sources)
                merge_fact(records, call, unit)
        for test in shard["tests"]:
            fields(test, ("package", "target_kind", "target_name", "name", "path", "line", "targets"))
            require(target_key(test) == target_key(unit) and unit["is_test"], "test has incorrect compiler unit context")
            text(test["name"])
            require(isinstance(test["targets"], list), "test targets must be an array")
            require(all(isinstance(target, str) and target for target in test["targets"]), "test target must be API ID")
            require(len(test["targets"]) == len(set(test["targets"])), f"duplicate test target: {test['name']}")
            source_location(root, test, generated_sources)
            value = {**test, "targets": sorted(test["targets"])}
            key = test_key(test)
            require(key not in tests or tests[key] == value, f"conflicting test definition: {key}")
            tests[key] = value
        for item in shard["unresolved"]:
            fields(item, ("caller", "path", "line", "reason"))
            text(item["caller"])
            text(item["reason"])
            source_location(root, item, generated_sources)
            merge_fact(unresolved, item, unit)
        require(isinstance(shard["coverage"], dict), "coverage must be an object")
        for name in ("bodies", "calls", "function_references", "external_calls"):
            require(name in shard["coverage"], f"missing compiler coverage counter: {name}")
        for name, count in shard["coverage"].items():
            require(type(count) is int and count >= 0, f"invalid compiler coverage counter: {name}")
            coverage[name] += count
    require(units, "no compiler shards produced; workspace units must be reanalyzed even with a warm Cargo cache")
    for call in [*calls.values(), *references.values()]:
        require(call["caller"] in apis, f"unknown call caller API: {call['caller']}")
        require(call["callee"] in apis, f"unknown call callee API: {call['callee']}")
        require(apis[call["caller"]]["package"] == call["caller_package"], "caller API package mismatch")
    for item in unresolved.values():
        require(item["caller"] in apis, f"unknown unresolved caller API: {item['caller']}")
    for test in tests.values():
        for target in test["targets"]:
            require(target in apis, f"unknown explicit test target API: {target}")
    return {"version": 3, "components": sorted_records(components), "apis": sorted_records(apis.values()),
            "calls": sorted_records(calls.values()), "references": sorted_records(references.values()),
            "tests": sorted_records(tests.values()),
            "units": sorted_records(units.values()), "coverage": dict(sorted(coverage.items())),
            "unresolved": sorted_records(unresolved.values()), "generated_sources": dict(sorted(generated_sources.items()))}


def execute(args, root, *, env=None):
    result = subprocess.run([str(arg) for arg in args], cwd=root, capture_output=True, text=True, env=env)
    require(result.returncode == 0,
            f"command failed ({result.returncode}): {shlex.join(map(str, args))}\n{result.stdout}{result.stderr}")
    return result.stdout


def cargo_target_kind(target):
    kinds = target["kind"]
    return "lib" if any(kind in ("lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro") for kind in kinds) else kinds[0]


def cargo_artifacts(outputs, metadata):
    packages = {package["id"]: package["name"] for package in metadata["packages"]
                if package["id"] in metadata["workspace_members"]}
    artifacts = {}
    for purpose, output in outputs:
        require(purpose in ("production", "tests"), f"unknown Cargo compilation purpose: {purpose}")
        for line in output.splitlines():
            if not line.strip():
                continue
            message = json.loads(line, object_pairs_hook=no_duplicate_keys)
            if message.get("reason") != "compiler-artifact" or message["package_id"] not in packages:
                continue
            target = message["target"]
            kind = cargo_target_kind(target)
            if kind == "custom-build":
                continue
            artifact = {"package": packages[message["package_id"]], "target_kind": kind,
                        "target_name": target["name"], "is_test": message["profile"]["test"],
                        "features": sorted(message["features"]), "executable": message.get("executable")}
            key = unit_key(artifact)
            if key in artifacts:
                previous = {name: value for name, value in artifacts[key].items() if name != "purposes"}
                require(previous == artifact, f"conflicting Cargo artifact: {key}")
                artifacts[key]["purposes"] = sorted(set(artifacts[key]["purposes"]) | {purpose})
            else:
                artifacts[key] = {**artifact, "purposes": [purpose]}
    require(artifacts, "Cargo produced no workspace compilation artifacts")
    return sorted_records(artifacts.values())


def compilation_purposes(shards, artifacts):
    purposes = {unit_key(artifact): artifact["purposes"] for artifact in artifacts}
    result = []
    for shard in shards:
        unit = shard["unit"]
        key = unit_key(unit)
        require(unit["target_kind"] == "custom-build" or key in purposes, f"compiler unit has no Cargo artifact: {key}")
        result.append({**shard, "unit": {**unit, "purposes": purposes.get(key, [])}})
    return result


def reconcile_units(bundle, artifacts):
    actual = {unit_key(unit) for unit in bundle["units"] if unit["target_kind"] != "custom-build"}
    expected = {unit_key(artifact) for artifact in artifacts}
    require(actual == expected,
            f"compiler unit coverage mismatch: missing shards {sorted(expected - actual)}; unexpected shards {sorted(actual - expected)}")


def parse_inventory(output):
    names = []
    for line in output.splitlines():
        if not line.strip():
            continue
        match = re.fullmatch(r"(.+): (test|benchmark)", line)
        require(match is not None, f"unsupported compiled test inventory line: {line}")
        if match[2] == "test":
            names.append(match[1])
    require(len(names) == len(set(names)), "compiled inventory contains duplicate test names")
    return set(names)


def check_inventory(bundle, artifacts, root):
    inventories = {}
    for artifact in artifacts:
        if not artifact["is_test"] or not artifact["executable"]:
            continue
        key = target_key(artifact)
        inventories[key] = parse_inventory(execute([artifact["executable"], "--list", "--format=terse"], root))
    expected = {}
    for test in bundle["tests"]:
        expected.setdefault(target_key(test), set()).add(test["name"])
    for key in set(inventories) | set(expected):
        actual, compiled = expected.get(key, set()), inventories.get(key, set())
        require(actual == compiled,
                f"compiled test inventory mismatch in {key}: compiler missing {sorted(compiled - actual)}; absent from binary {sorted(actual - compiled)}")
    return inventories


def excluded_targets(metadata, artifacts):
    compiled = {target_key(artifact) for artifact in artifacts}
    result = []
    for package in metadata["packages"]:
        if package["id"] not in metadata["workspace_members"]:
            continue
        for target in package["targets"]:
            kind = cargo_target_kind(target)
            key = package["name"], kind, target["name"]
            if kind != "custom-build" and key not in compiled:
                result.append({"package": package["name"], "target_kind": kind, "target_name": target["name"],
                               "required_features": target.get("required-features", []),
                               "reason": "not compiled by this Cargo configuration"})
    return sorted_records(result)


def toolchain(root):
    path = root / "tools/traceability/rust-toolchain.toml"
    require(path.is_file(), f"missing compiler toolchain pin: {path}")
    return tomllib.loads(path.read_text(encoding="utf-8"))["toolchain"]["channel"]


def load_traces(root):
    root = Path(root).resolve()
    channel = toolchain(root)
    cargo = ["cargo", "+" + channel]
    prerequisites = f"Install the pinned compiler components with: rustup toolchain install {channel} --component rustc-dev --component llvm-tools-preview"
    try:
        execute(cargo + ["build", "--locked", "--manifest-path", root / "tools/traceability/Cargo.toml",
                         "--target-dir", root / "target/traceability-driver"], root)
    except InvalidTrace as error:
        raise InvalidTrace(f"{error}\n{prerequisites}") from error
    driver = root / "target/traceability-driver/debug/traceability-collector"
    require(driver.is_file(), f"compiler driver binary missing: {driver}")
    metadata = json.loads(execute(cargo + ["metadata", "--locked", "--format-version", "1"], root),
                          object_pairs_hook=no_duplicate_keys)
    components = []
    for package in metadata["packages"]:
        if package["id"] in metadata["workspace_members"]:
            directory = Path(package["manifest_path"]).resolve().parent
            require(directory.is_relative_to(root), f"workspace package outside repository: {directory}")
            components.append({"id": package["name"], "package": package["name"], "path": directory.relative_to(root).as_posix()})
    analysis = root / "target/traceability-analysis"
    analysis.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="run-", dir=analysis) as directory:
        run = Path(directory)
        shards = run / "shards"
        shards.mkdir()
        metadata_path = run / "metadata.json"
        metadata_path.write_text(canonical(metadata), encoding="utf-8")
        wrapper = run / "workspace-wrapper"
        wrapper.symlink_to(driver)
        sysroot = execute(["rustc", "+" + channel, "--print", "sysroot"], root).strip()
        environment = {**os.environ, "RUSTC_WORKSPACE_WRAPPER": str(wrapper), "TRACEABILITY_ROOT": str(root),
                       "TRACEABILITY_SHARDS": str(shards), "TRACEABILITY_METADATA": str(metadata_path),
                       "LD_LIBRARY_PATH": str(Path(sysroot) / "lib") + os.pathsep + os.environ.get("LD_LIBRARY_PATH", "")}
        commands = [["build", "--workspace", "--lib", "--bins", "--locked"],
                    ["test", "--workspace", "--all-targets", "--no-run", "--locked"]]
        outputs = [execute(cargo + command + ["--message-format=json", "--target-dir", analysis], root, env=environment)
                   for command in commands]
        artifacts = cargo_artifacts(zip(("production", "tests"), outputs), metadata)
        captures = compilation_purposes([read_json(path) for path in sorted(shards.glob("*.json"))], artifacts)
        bundle = merge_shards(root, captures, components)
        reconcile_units(bundle, artifacts)
        check_inventory(bundle, artifacts, root)
    bundle["excluded_targets"] = excluded_targets(metadata, artifacts)
    bundle["configuration"] = {"toolchain": channel, "cargo": [shlex.join(command) for command in commands],
                               "scope": "Current host and default Cargo features; inactive cfg branches are outside this snapshot."}
    return bundle


def selected_tests(bundle, kind="all"):
    return [test for test in bundle["tests"] if kind == "all" or bool(test["targets"]) == (kind == "targeted")]


def cargo_command(test, channel):
    target = ["--lib"] if test["target_kind"] == "lib" else ["--" + test["target_kind"], test["target_name"]]
    return ["cargo", "+" + channel, "test", "--locked", "-p", test["package"], *target]


def run_tests(tests, root, channel):
    for test in tests:
        args = cargo_command(test, channel) + [test["name"], "--", "--exact", "--format", "pretty", "--color", "never"]
        output = execute(args, root)
        summary = re.findall(r"^test result: (.+)$", output, flags=re.MULTILINE)
        require(len(summary) == 1 and re.fullmatch(r"ok\. 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9.]+s", summary[0]),
                f"expected one passed exact test: {test_key(test)}\n{output}")
        require(re.search(r"^test " + re.escape(test["name"]) + r" \.\.\. ok$", output, flags=re.MULTILINE),
                f"exact test identity not confirmed: {test_key(test)}\n{output}")
        print(f"PASS {' / '.join(test_key(test))}")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("check", "run", "viewer"))
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--kind", choices=("all", "targeted", "untargeted"), default="all")
    parser.add_argument("--output", type=Path, help="HTML destination for viewer or JSON destination for check")
    args = parser.parse_args(argv)
    if args.command == "run" and args.output is not None:
        parser.error("--output is only available for check and viewer")
    root = args.root.resolve()
    try:
        bundle = load_traces(root)
        if args.command == "viewer":
            from traceability_viewer import write_viewer
            destination = args.output or root / "target/traceability-viewer.html"
            write_viewer(root, bundle, destination)
            print(f"Viewer snapshot: {destination.resolve()}")
        elif args.command == "run":
            run_tests(selected_tests(bundle, args.kind), root, bundle["configuration"]["toolchain"])
        else:
            if args.output:
                from traceability_viewer import capture, protect_destination
                destination = protect_destination(root, args.output, capture(root, bundle))
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_text(json.dumps(bundle, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            print(f"Compiler traceability matches {len(bundle['units'])} units and {len(bundle['tests'])} compiled tests; "
                  f"{len(bundle['calls'])} workspace calls; {len(bundle['unresolved'])} unresolved calls.")
        return 0
    except (InvalidTrace, OSError, ValueError, KeyError, TypeError) as error:
        print(f"traceability: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
