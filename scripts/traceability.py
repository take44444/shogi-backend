#!/usr/bin/env python3
"""Validate source-backed feature graphs and export the HTML viewer."""
import argparse
from collections import Counter
import json
import os
from pathlib import Path, PurePosixPath
import re
import shlex
import subprocess
import sys


ID = re.compile(r"[a-z][a-z0-9-]*\Z")
CARGO_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_-]*\Z")
TEST_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*\Z")


class InvalidTrace(Exception):
    pass


def require(condition, message):
    if not condition:
        raise InvalidTrace(message)


def fields(value, required, optional=()):
    require(isinstance(value, dict), "expected an object")
    require(set(required) <= value.keys() <= set(required) | set(optional),
            f"expected fields {sorted(required)}, optional {sorted(optional)}; got {sorted(value)}")


def text(value):
    require(isinstance(value, str) and bool(value.strip()) and "\n" not in value and "\r" not in value,
            "expected nonempty single-line text")
    return value


def sequence(value, *, nonempty=True):
    require(isinstance(value, list) and (bool(value) or not nonempty), "expected a nonempty list")
    return value


def unique(values, context):
    require(len(values) == len(set(values)), f"{context}: duplicate association or identity")


def no_duplicate_keys(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON field: {key}")
        result[key] = value
    return result


def source_location(source, root, *, parsed_test=False):
    fields(source, ("path", "needle", "line") if parsed_test else ("path", "needle"))
    path, needle = text(source["path"]), text(source["needle"])
    relative = PurePosixPath(path)
    require(not relative.is_absolute() and ".." not in relative.parts and str(relative) == path,
            f"source path must be repository-relative: {path}")
    resolved = (root / path).resolve()
    require(resolved.is_relative_to(root) and resolved.is_file(), f"source file missing or outside repository: {path}")
    content = resolved.read_text(encoding="utf-8")
    if parsed_test:
        line = source["line"]
        content_lines = content.splitlines()
        require(type(line) is int and 0 < line <= len(content_lines), "invalid parsed Rust source line")
        require(content_lines[line - 1].strip() == needle, f"parsed Rust source line changed: {path}:{line}")
        return line
    require(content.count(needle) == 1, f"source needle must occur exactly once: {path}: {needle}")
    return content[:content.index(needle)].count("\n") + 1


def validate_graph(root, graph, *, parsed_tests=False):
    fields(graph, ("version", "feature", "guarantees", "contracts", "tests", "limitations"))
    require(type(graph["version"]) is int and graph["version"] == 1, "unsupported version")
    fields(graph["feature"], ("id", "title", "source", "flow"))
    text(graph["feature"]["title"])
    for step in sequence(graph["feature"]["flow"]):
        text(step)
    for limitation in sequence(graph["limitations"]):
        text(limitation)
    nodes = [graph["feature"]]
    for group, required, optional in (
        ("guarantees", ("id", "statement", "contracts"), ()),
        ("contracts", ("id", "statement", "source", "evidence"), ("gap",)),
        ("tests", ("id", "package", "target", "name", "source", "scope"), ()),
    ):
        for node in sequence(graph[group], nonempty=group != "tests"):
            fields(node, required, optional)
            nodes.append(node)
    for node in nodes:
        require(ID.fullmatch(text(node["id"])), f"invalid ID: {node['id']}")
    unique([node["id"] for node in nodes], "graph IDs")
    contracts = {node["id"]: node for node in graph["contracts"]}
    tests = {node["id"]: node for node in graph["tests"]}
    used_contracts, used_tests = set(), set()
    for guarantee in graph["guarantees"]:
        text(guarantee["statement"])
        refs = [text(ref) for ref in sequence(guarantee["contracts"])]
        unique(refs, guarantee["id"])
        require(set(refs) <= contracts.keys(), f"{guarantee['id']}: unknown contract")
        used_contracts.update(refs)
    for contract in contracts.values():
        text(contract["statement"])
        if "gap" in contract:
            text(contract["gap"])
        evidence = sequence(contract["evidence"], nonempty=False)
        require(evidence or "gap" in contract, f"{contract['id']}: evidence or explicit gap required")
        for edge in evidence:
            fields(edge, ("test", "relationship", "reason"))
            require(text(edge["test"]) in tests, f"{contract['id']}: unknown test")
            require(edge["relationship"] in ("asserts", "exercises", "unreviewed"), f"{contract['id']}: invalid relationship")
            text(edge["reason"])
            used_tests.add(edge["test"])
        require(any(edge["relationship"] in ("asserts", "exercises") for edge in evidence) or "gap" in contract,
                f"{contract['id']}: evidence or explicit gap required; unreviewed ownership is insufficient")
        unique([edge["test"] for edge in evidence], contract["id"])
    require(used_contracts == contracts.keys(), "orphan contract")
    require(used_tests == tests.keys(), "orphan test")
    for test in tests.values():
        require(CARGO_NAME.fullmatch(text(test["package"])), f"{test['id']}: invalid package")
        fields(test["target"], ("kind",), ("name",))
        kind = test["target"]["kind"]
        require(kind in ("lib", "test", "bin"), f"{test['id']}: unsupported target kind")
        require((kind != "lib") == ("name" in test["target"]), f"{test['id']}: test and bin targets need name; lib has none")
        if kind != "lib":
            require(CARGO_NAME.fullmatch(text(test["target"]["name"])), f"{test['id']}: invalid target name")
        require(TEST_NAME.fullmatch(text(test["name"])), f"{test['id']}: expected a complete Rust test identity")
        fields(test["source"], ("path", "needle", "line") if parsed_tests else ("path", "needle"))
        require(f"fn {test['name'].split('::')[-1]}(" in text(test["source"]["needle"]),
                f"{test['id']}: source needle must name the declared test function")
        text(test["scope"])
    unique([(*target_key(test), test["name"]) for test in tests.values()], "Cargo tests")
    lines = {node["id"]: source_location(node["source"], root, parsed_test=parsed_tests and node["id"] in tests) for node in nodes if "source" in node}
    return graph, lines


class Traces(list):
    def __init__(self, bundle):
        super().__init__()
        self.directory_bundle = bundle


def directory_traces(root):
    repository = Path(__file__).resolve().parent.parent
    manifest = repository / "tools/traceability/Cargo.toml"
    args = ["cargo", "run", "--quiet", "--locked", "--manifest-path", str(manifest)]
    if "CARGO_TARGET_DIR" not in os.environ:
        args += ["--target-dir", str(repository / "target")]
    output = execute(args + ["--", str(root)], root)
    bundle = json.loads(output, object_pairs_hook=no_duplicate_keys)
    fields(bundle, ("graphs", "inventories", "classifications", "entrypoints", "surfaces", "excluded_targets", "packages", "examples"))
    return bundle


def load_traces(root):
    bundle = directory_traces(root)
    require(bundle.get("graphs"), "no feature declarations under workspace src/trace/ or tests/trace/")
    traces = Traces(bundle)
    for fragment in bundle.get("graphs", []):
        fields(fragment, ("record", "graph"))
        try:
            graph, lines = validate_graph(root, fragment["graph"], parsed_tests=True)
            traces.append((Path(fragment["record"]), graph, lines))
        except (InvalidTrace, OSError, ValueError) as error:
            raise InvalidTrace(f"{fragment['record']}: {error}") from error
    unique([graph["feature"]["id"] for _, graph, _ in traces], "global feature IDs")
    traces.sort(key=lambda trace: trace[1]["feature"]["id"])
    return traces


def target_key(test):
    return test["package"], test["target"]["kind"], test["target"].get("name")


def cargo_command(test):
    args = ["cargo", "test", "--locked", "-p", test["package"]]
    target = test["target"]
    return args + (["--lib"] if target["kind"] == "lib" else ["--" + target["kind"], target["name"]])


def execute(args, root):
    result = subprocess.run(args, cwd=root, capture_output=True, text=True)
    require(result.returncode == 0,
            f"command failed ({result.returncode}): {shlex.join(args)}\n{result.stdout}{result.stderr}")
    return result.stdout


def check_surfaces(traces, root):
    bundle = traces.directory_bundle
    features = {graph["feature"]["id"] for _, graph, _ in traces}
    owned_features = set()

    def own(feature, label):
        require(text(feature) in features, f"{label}: unknown feature owner {feature}")
        owned_features.add(feature)

    openapi, grpc = {}, {}
    for surface in sequence(bundle["surfaces"], nonempty=False):
        kind = surface.get("kind")
        if kind == "OpenApi":
            fields(surface, ("kind", "feature", "package", "example", "operation"))
            package, example, operation = (text(surface[key]) for key in ("package", "example", "operation"))
            require(CARGO_NAME.fullmatch(package) and CARGO_NAME.fullmatch(example), "invalid OpenAPI package or example")
            require(re.fullmatch(r"(GET|PUT|POST|DELETE|OPTIONS|HEAD|PATCH|TRACE) /\S*", operation),
                    f"invalid OpenAPI operation: {operation}")
            key = package, example
            expected = openapi.setdefault(key, {})
            require(operation not in expected, f"duplicate OpenAPI operation owner: {key} {operation}")
            expected[operation] = surface["feature"]
            own(surface["feature"], f"{package}: {operation}")
        elif kind == "Grpc":
            fields(surface, ("kind", "feature", "path", "service", "operation"))
            path, service, operation = (text(surface[key]) for key in ("path", "service", "operation"))
            require(all(re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", item) for item in (service, operation)), "invalid gRPC service or operation")
            key = path, service, operation
            require(key not in grpc, f"duplicate gRPC operation owner: {key}")
            grpc[key] = surface["feature"]
            own(surface["feature"], f"{service}.{operation}")
        else:
            require(False, f"unsupported surface kind: {kind}")

    exporters, example_packages, example_identities = set(), set(), set()
    for entry in sequence(bundle["examples"], nonempty=False):
        fields(entry, ("package", "example", "source", "kind", "reason"))
        package, example, source, kind, _ = (text(entry[key]) for key in ("package", "example", "source", "kind", "reason"))
        require(CARGO_NAME.fullmatch(package) and CARGO_NAME.fullmatch(example), "invalid Cargo example identity")
        identity = package, example
        require(identity not in example_identities, f"duplicate Cargo example: {identity}")
        example_identities.add(identity)
        relative = PurePosixPath(source)
        require(not relative.is_absolute() and ".." not in relative.parts and str(relative) == source,
                f"example source must be repository-relative: {source}")
        resolved = (root / source).resolve()
        require(resolved.is_relative_to(root) and resolved.is_file(), f"example source missing or outside repository: {source}")
        require(kind in ("OpenApi", "Program"), f"unsupported example kind: {kind}")
        example_packages.add(package)
        if kind == "OpenApi":
            exporters.add(identity)
    for package in sorted(example_packages):
        execute(["cargo", "check", "--locked", "-p", package, "--examples"], root)
    require(set(openapi) == exporters,
            f"OpenAPI exporter set mismatch: missing {sorted(exporters - openapi.keys())}; extra {sorted(openapi.keys() - exporters)}")
    methods = {"get", "put", "post", "delete", "options", "head", "patch", "trace"}
    for (package, example), operations in sorted(openapi.items()):
        output = execute(["cargo", "run", "--quiet", "--locked", "-p", package, "--example", example], root)
        document = json.loads(output, object_pairs_hook=no_duplicate_keys)
        require(isinstance(document, dict) and isinstance(document.get("paths"), dict), "OpenAPI output needs paths")
        actual = set()
        for path, item in document["paths"].items():
            require(isinstance(path, str) and path.startswith("/") and isinstance(item, dict), "invalid OpenAPI path item")
            require("$ref" not in item, "unsupported referenced OpenAPI path item")
            for method in item.keys() & methods:
                require(isinstance(item[method], dict), "invalid OpenAPI operation object")
                actual.add(f"{method.upper()} {path}")
        require(actual == set(operations),
                f"{package}: OpenAPI operation set mismatch: uncataloged {sorted(actual - operations.keys())}; absent {sorted(operations.keys() - actual)}")

    proto_files = set()
    for directory in [root / "proto"] + [root / package["path"] / "proto" for package in bundle["packages"]]:
        if directory.is_dir():
            for path in directory.rglob("*.proto"):
                resolved = path.resolve()
                require(resolved.is_relative_to(root), f"proto source outside repository: {path}")
                proto_files.add(resolved)
    actual_grpc = set()
    declaration = r"\brpc\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(\s*(?:stream\s+)?[.\w]+\s*\)\s*returns\s*\(\s*(?:stream\s+)?[.\w]+\s*\)\s*;"
    for path in sorted(proto_files):
        proto = re.sub(r"/\*.*?\*/|//[^\n]*", "", path.read_text(encoding="utf-8"), flags=re.DOTALL)
        blocks = re.findall(r"\bservice\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{([^{}]*)\}", proto)
        require(len(blocks) == len(re.findall(r"\bservice\b", proto)), f"missing or unsupported gRPC service block: {path.relative_to(root)}")
        unique([service for service, _ in blocks], "gRPC services")
        for service, block in blocks:
            names = re.findall(declaration, block)
            require(names and not re.sub(declaration, "", block).strip(), f"unsupported gRPC declarations: {service}")
            unique(names, "gRPC operations")
            actual_grpc.update((path.relative_to(root).as_posix(), service, name) for name in names)
    require(actual_grpc == set(grpc),
            f"gRPC operation set mismatch: uncataloged {sorted(actual_grpc - grpc.keys())}; absent {sorted(grpc.keys() - actual_grpc)}")

    entry_names = []
    for entry in sequence(bundle["entrypoints"], nonempty=False):
        fields(entry, ("feature", "name", "source"))
        name = text(entry["name"])
        source_location(entry["source"], root)
        own(entry["feature"], name)
        entry_names.append(name)
    unique(entry_names, "entrypoint names")
    require(features == owned_features, f"orphan feature without surface or entrypoint: {sorted(features - owned_features)}")


def mapped_tests(traces):
    tests = {}
    for _, graph, _ in traces:
        for test in graph["tests"]:
            key = (*target_key(test), test["name"])
            if key not in tests:
                tests[key] = (test, [])
            tests[key][1].append(f"{graph['feature']['id']}/{test['id']}")
    return tests.values()


def cargo_inventory(test, root):
    args = cargo_command(test) + ["--", "--list", "--format", "terse"]
    output = execute(args, root)
    names = []
    for line in output.splitlines():
        if not line.strip():
            continue
        match = re.fullmatch(r"(.+): (test|benchmark)", line)
        require(match is not None, f"unsupported Cargo inventory: {shlex.join(args)}\n{output}")
        if match[2] == "test":
            names.append(match[1])
    return Counter(names)


def check_inventory(tests, root, directory_bundle=None):
    inventories = {}
    for test, _ in tests:
        key = target_key(test)
        if key not in inventories:
            inventories[key] = cargo_inventory(test, root)
        require(inventories[key][test["name"]] == 1,
                f"{test['id']}: expected exactly one compiled test in {key}: {test['name']}")
    for inventory in (directory_bundle or {}).get("inventories", []):
        key = target_key(inventory)
        if key not in inventories:
            inventories[key] = cargo_inventory(inventory, root)
        actual = inventories[key]
        expected = Counter(inventory["tests"])
        require(actual == expected,
                f"global classification inventory mismatch in {key}: "
                f"unclassified compiled {sorted((actual - expected).elements())}; "
                f"parsed but absent {sorted((expected - actual).elements())}")


def run_tests(tests, root):
    for test, aliases in tests:
        args = cargo_command(test) + [test["name"], "--", "--exact", "--format", "pretty", "--color", "never"]
        output = execute(args, root)
        failure = f"{test['id']}: expected one passed exact test, zero failed/ignored\n{output}"
        frames, runs = [], []
        for line in output.splitlines():
            start = re.fullmatch(r"running ([0-9]+) tests?", line)
            event = re.fullmatch(r"test (.+) \.\.\. (.+)", line)
            pending = re.fullmatch(r"test (.+) \.\.\. ", line)
            if start:
                frames.append({"count": int(start[1]), "events": [], "pending": None})
            elif line.startswith("test result: "):
                result = line.removeprefix("test result: ")
                require(frames and re.fullmatch(
                    r"(?:ok|FAILED)\. [0-9]+ passed; [0-9]+ failed; [0-9]+ ignored; [0-9]+ measured; [0-9]+ filtered out; finished in [0-9.]+s",
                    result), failure)
                frame = frames.pop()
                require(frame["pending"] is None, failure)
                frame["summary"] = result
                if not frames:
                    runs.append(frame)
            elif event:
                require(frames and frames[-1]["pending"] is None, failure)
                frames[-1]["events"].append((event[1], event[2]))
            elif pending:
                require(frames and frames[-1]["pending"] is None, failure)
                frames[-1]["pending"] = pending[1]
            elif re.fullmatch(r"ok|FAILED|ignored(?:, .+)?", line):
                require(frames and frames[-1]["pending"] is not None, failure)
                frames[-1]["events"].append((frames[-1]["pending"], line))
                frames[-1]["pending"] = None
            elif line.startswith(("running ", "test ")):
                require(False, failure)
        summary = r"ok\. 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9.]+s"
        require(not frames and len(runs) == 1 and runs[0]["count"] == 1
                and runs[0]["events"] == [(test["name"], "ok")]
                and re.fullmatch(summary, runs[0]["summary"]), failure)
        print(f"PASS {', '.join(aliases)}: {shlex.join(args)}")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("check", "run", "viewer"))
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--output", type=Path, help="viewer HTML destination (viewer only)")
    args = parser.parse_args(argv)
    if args.output is not None and args.command != "viewer":
        parser.error("--output is only available for viewer")
    root = args.root.resolve()
    try:
        traces = load_traces(root)
        check_surfaces(traces, root)
        tests = list(mapped_tests(traces))
        check_inventory(tests, root, traces.directory_bundle)
        if args.command == "viewer":
            from traceability_viewer import write_viewer

            destination = args.output if args.output is not None else root / "target/traceability-viewer.html"
            write_viewer(root, traces, destination)
            print(f"Viewer snapshot for {len(traces)} features: {destination.resolve()}")
            return 0
        if args.command == "run":
            run_tests(tests, root)
        else:
            print(f"Traceability surfaces, structure, and Cargo identities match: {len(traces)} features")
        return 0
    except (InvalidTrace, OSError, ValueError) as error:
        print(f"traceability: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
