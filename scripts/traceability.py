#!/usr/bin/env python3
"""Validate component contracts and test identities, and export the HTML viewer."""
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


def repository_file(root, path):
    relative = PurePosixPath(text(path))
    require(not relative.is_absolute() and ".." not in relative.parts and str(relative) == path,
            f"source path must be repository-relative: {path}")
    resolved = (root / path).resolve()
    require(resolved.is_relative_to(root) and resolved.is_file(), f"source file missing or outside repository: {path}")
    return resolved


def cargo_target(value):
    fields(value, ("kind",), ("name",))
    require(value["kind"] in ("lib", "test", "bin"), "unsupported target kind")
    require((value["kind"] != "lib") == ("name" in value), "test and bin targets need name; lib has none")
    if "name" in value:
        require(CARGO_NAME.fullmatch(text(value["name"])), "invalid target name")


def strings(value, *, nonempty=False):
    return [text(item) for item in sequence(value, nonempty=nonempty)]


def cargo_production_dependencies(root):
    if not (root / "Cargo.toml").is_file():
        return None
    metadata = json.loads(execute(["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], root),
                          object_pairs_hook=no_duplicate_keys)
    return {package["name"]: {dependency["name"] for dependency in package["dependencies"]
                              if dependency.get("kind") is None}
            for package in metadata["packages"]}


def validate_bundle(root, bundle):
    root = Path(root).resolve()
    fields(bundle, ("version", "components", "features", "contracts", "tests", "inventories", "packages", "examples", "excluded_targets"))
    require(type(bundle["version"]) is int and bundle["version"] == 2, "unsupported version")
    for key in ("components", "features", "contracts", "tests", "inventories", "packages", "examples", "excluded_targets"):
        sequence(bundle[key], nonempty=False)
    packages = {}
    for package in bundle["packages"]:
        fields(package, ("name", "path"))
        name, path = text(package["name"]), text(package["path"])
        require(CARGO_NAME.fullmatch(name), f"invalid package: {name}")
        relative = PurePosixPath(path)
        require(not relative.is_absolute() and ".." not in relative.parts and str(relative) == path, "invalid package path")
        require(name not in packages, "duplicate package")
        resolved = (root / path).resolve()
        require(resolved.is_relative_to(root) and resolved.is_dir(), f"package directory missing or outside repository: {path}")
        packages[name] = path
    unique(list(packages.values()), "package paths")

    def belongs(path, owner):
        resolved = (root / path).resolve()
        matches = [package for package, directory in packages.items() if resolved.is_relative_to((root / directory).resolve())]
        return bool(matches) and max(matches, key=lambda package: len(PurePosixPath(packages[package]).parts)) == owner

    def anchor(source, owner=None, *, parsed=False):
        source_location(source, root, parsed_test=parsed)
        if owner is not None:
            require(belongs(source["path"], owner), f"source ownership mismatch: {source['path']} must belong to {owner}")

    def record(node, category):
        owner = text(node["owner"])
        require(owner in packages, f"unknown declaration owner: {owner}")
        path = text(node["record"])
        repository_file(root, path)
        require(belongs(path, owner), f"declaration source ownership mismatch: {path}")
        base = PurePosixPath(packages[owner]) / "src/trace" / category
        require(PurePosixPath(path).parent == base and path.endswith(".rs"), f"invalid {category} declaration path: {path}")
        return owner

    components, actors = {}, set()
    for component in bundle["components"]:
        fields(component, ("id", "package", "path", "description", "notes", "external", "operations", "entrypoints", "record"))
        identity = text(component["id"])
        require(identity in packages and component["package"] == identity and component["path"] == packages[identity], "component must match Cargo package")
        require(identity not in components, "duplicate component ID")
        text(component["description"])
        strings(component["notes"])
        if component["record"] is not None:
            path = component["record"]
            require(path == str(PurePosixPath(packages[identity]) / "src/trace/component.rs"), "invalid component declaration path")
            repository_file(root, path)
        sequence(component["operations"], nonempty=False)
        sequence(component["entrypoints"], nonempty=False)
        for external in sequence(component["external"], nonempty=False):
            fields(external, ("id", "title"))
            actor = text(external["id"])
            require(ID.fullmatch(actor), "invalid external component ID")
            require(actor not in actors and actor not in packages, f"duplicate external component ID: {actor}")
            text(external["title"])
            actors.add(actor)
        names = []
        for entry in component["entrypoints"]:
            fields(entry, ("name", "source"))
            names.append(text(entry["name"]))
            anchor(entry["source"], identity)
        unique(names, "component entrypoints")
        components[identity] = component
    require(components.keys() == packages.keys(), "components must cover every Cargo package")
    participants = set(components) | actors
    contracts = {}
    for contract in bundle["contracts"]:
        fields(contract, ("id", "owner", "record", "consumer", "provider", "interface", "statement", "assumptions", "limitations", "source", "call", "implementation", "binding", "requires", "gap"))
        owner = record(contract, "contracts")
        identity = text(contract["id"])
        require(identity.startswith(owner + "/") and ID.fullmatch(identity[len(owner) + 1:]), f"invalid global contract ID: {identity}")
        require(identity not in contracts, "duplicate contract ID")
        consumer, provider, interface = (text(contract[key]) for key in ("consumer", "provider", "interface"))
        require(consumer in participants and provider in participants, f"{identity}: unknown component")
        require(consumer != provider, f"{identity}: same component contract")
        require(interface in ("RustApi", "Port", "Http", "Grpc", "WebSocket", "External"), f"{identity}: invalid interface")
        require(consumer in components or provider in components, f"{identity}: contract needs workspace participant")
        if interface == "Port":
            require(consumer in components and provider in components, f"{identity}: Port parties must be workspace components")
        elif interface == "External":
            require(consumer in components and provider in actors and owner == consumer, f"{identity}: External owner must be local consumer")
        else:
            require(provider in components and owner == provider, f"{identity}: API owner must be provider")
        text(contract["statement"])
        strings(contract["assumptions"])
        strings(contract["limitations"])
        anchor(contract["source"], owner)
        if contract["gap"] is not None:
            text(contract["gap"])
        for key in ("call", "implementation", "binding"):
            if contract[key] is not None:
                anchor(contract[key])
        if consumer in components:
            require(contract["call"] is not None, f"{identity}: workspace consumer call anchor required")
            anchor(contract["call"], consumer)
        if contract["implementation"] is not None:
            require(provider in components, f"{identity}: external provider has no workspace implementation")
            anchor(contract["implementation"], provider)
        if interface == "Port":
            require(contract["implementation"] is not None and contract["binding"] is not None, f"{identity}: Port implementation and binding required")
            require(any(belongs(contract["binding"]["path"], package) for package in packages), f"{identity}: binding must belong to workspace component")
        sequence(contract["requires"], nonempty=False)
        contracts[identity] = contract

    dependencies = cargo_production_dependencies(root) if any(contract["interface"] == "Port" for contract in contracts.values()) else None
    for contract in contracts.values():
        identity = contract["id"]
        if contract["interface"] == "Port" and dependencies is not None:
            require(contract["owner"] in dependencies.get(contract["provider"], set()),
                    f"{identity}: Port provider must have production dependency on interface owner")
        children = []
        for dependency in contract["requires"]:
            fields(dependency, ("contract", "reason"))
            child = text(dependency["contract"])
            text(dependency["reason"])
            require(child in contracts, f"{identity}: unknown required contract {child}")
            require(child != identity, f"{identity}: self reference")
            require(contract["provider"] == contracts[child]["consumer"], f"{identity}: nonadjacent required contract {child}")
            children.append(child)
        unique(children, f"{identity} requires")
    active, finished = set(), set()

    def visit(identity):
        require(identity not in active, f"contract cycle: {identity}")
        if identity in finished:
            return
        active.add(identity)
        for dependency in contracts[identity]["requires"]:
            visit(dependency["contract"])
        active.remove(identity)
        finished.add(identity)

    for identity in contracts:
        visit(identity)
    roots, feature_ids = set(), []
    for feature in bundle["features"]:
        fields(feature, ("id", "title", "source", "flow", "guarantees", "limitations", "owner", "record"))
        owner = record(feature, "features")
        identity = text(feature["id"])
        require(ID.fullmatch(identity), "invalid feature ID")
        feature_ids.append(identity)
        text(feature["title"])
        anchor(feature["source"], owner)
        strings(feature["flow"], nonempty=True)
        strings(feature["limitations"])
        guarantee_ids = []
        for guarantee in sequence(feature["guarantees"]):
            fields(guarantee, ("id", "statement", "roots"))
            require(ID.fullmatch(text(guarantee["id"])), "invalid guarantee ID")
            guarantee_ids.append(guarantee["id"])
            text(guarantee["statement"])
            refs = strings(guarantee["roots"], nonempty=True)
            unique(refs, "guarantee roots")
            for ref in refs:
                require(ref in contracts, f"{identity}: unknown root contract {ref}")
                contract = contracts[ref]
                require(contract["consumer"] == "frontend" and contract["provider"] in components
                        and contract["interface"] in ("Http", "WebSocket", "Grpc"), f"{identity}: feature root must be frontend boundary")
                roots.add(ref)
        unique(guarantee_ids, f"{identity} guarantees")
    unique(feature_ids, "global feature IDs")
    operation_contracts, operation_keys = set(), set()
    for component in components.values():
        for operation in component["operations"]:
            fields(operation, ("contract", "audience", "surface", "reason"))
            ref = text(operation["contract"])
            require(ref in contracts and contracts[ref]["provider"] == component["id"], f"operation needs contract provided by component: {ref}")
            audience = operation["audience"]
            require(audience in ("Frontend", "Internal", "Operator"), "invalid operation audience")
            require(isinstance(operation["reason"], str) and "\n" not in operation["reason"] and "\r" not in operation["reason"], "operation reason must be single-line text")
            if audience == "Frontend":
                require(ref in roots, f"frontend operation must be feature root: {ref}")
            else:
                text(operation["reason"])
                require(ref not in roots, f"internal/operator operation cannot be feature root: {ref}")
            surface = operation["surface"]
            require(isinstance(surface, dict), "expected operation surface object")
            kind = surface.get("kind")
            if kind == "OpenApi":
                fields(surface, ("kind", "package", "example", "operation"))
                require(surface["package"] == component["id"], "OpenAPI operation package must match component")
                require(CARGO_NAME.fullmatch(text(surface["example"])), "invalid OpenAPI example")
                require(re.fullmatch(r"(GET|PUT|POST|DELETE|OPTIONS|HEAD|PATCH|TRACE) /\S*", text(surface["operation"])), "invalid OpenAPI operation")
                expected_interface = "Http"
                key = kind, surface["package"], surface["example"], surface["operation"]
            elif kind == "Grpc":
                fields(surface, ("kind", "path", "service", "operation"))
                repository_file(root, surface["path"])
                require(all(re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", text(surface[field])) for field in ("service", "operation")), "invalid gRPC operation")
                expected_interface = "Grpc"
                key = kind, surface["path"], surface["service"], surface["operation"]
            elif kind == "WebSocket":
                fields(surface, ("kind", "path", "command", "source"))
                text(surface["path"])
                text(surface["command"])
                anchor(surface["source"], component["id"])
                expected_interface = "WebSocket"
                key = kind, component["id"], surface["path"], surface["command"]
            else:
                raise InvalidTrace(f"unsupported surface kind: {kind}")
            require(contracts[ref]["interface"] == expected_interface, "operation interface mismatch")
            require(key not in operation_keys, f"duplicate operation owner: {key}")
            operation_keys.add(key)
            operation_contracts.add(ref)
    require(roots <= operation_contracts, f"feature root lacks declared operation: {sorted(roots - operation_contracts)}")
    asserted, identities, test_ids = set(), [], []
    for test in bundle["tests"]:
        fields(test, ("id", "package", "target", "name", "source", "classification", "semantics"))
        text(test["id"])
        test_ids.append(test["id"])
        require(text(test["package"]) in components, "unknown test package")
        cargo_target(test["target"])
        require(TEST_NAME.fullmatch(text(test["name"])), "expected a complete Rust test identity")
        fields(test["source"], ("path", "needle", "line"))
        require(f"fn {test['name'].split('::')[-1]}(" in text(test["source"]["needle"]), "source needle must name the declared test function")
        anchor(test["source"], test["package"], parsed=True)
        identities.append((*target_key(test), test["name"]))
        classification, semantics = test["classification"], test["semantics"]
        require(classification in ("boundary", "internal", "infrastructure", "helper"), "unknown test classification")
        if classification == "boundary":
            require(test["target"]["kind"] == "test" and PurePosixPath(test["source"]["path"]).is_relative_to(PurePosixPath(packages[test["package"]]) / "tests/contracts"), "boundary tests must be integration tests under tests/contracts/")
            fields(semantics, ("evidence",))
            refs = []
            for evidence in sequence(semantics["evidence"]):
                fields(evidence, ("contract", "relationship", "scope", "reason", "observation", "real", "fake"))
                ref = text(evidence["contract"])
                require(ref in contracts, f"unknown evidence contract: {ref}")
                refs.append(ref)
                require(evidence["relationship"] in ("asserts", "exercises", "unreviewed"), "invalid relationship")
                text(evidence["scope"])
                text(evidence["reason"])
                require(evidence["observation"] in ("ProviderBoundary", "CompleteInteraction"), "invalid observation")
                real, fake = strings(evidence["real"]), strings(evidence["fake"])
                unique(real, "real participants")
                unique(fake, "fake participants")
                require(set(real + fake) <= participants, "unknown evidence participant")
                require(not set(real) & set(fake), "real and fake participants overlap")
                require(contracts[ref]["provider"] in real, f"{ref}: evidence provider must be real")
                if evidence["observation"] == "CompleteInteraction":
                    require(contracts[ref]["consumer"] in real, f"{ref}: complete interaction consumer must be real")
                if evidence["relationship"] == "asserts":
                    asserted.add(ref)
            unique(refs, "test evidence")
        elif classification == "internal":
            fields(semantics, ())
            path = PurePosixPath(test["source"]["path"])
            package_path = PurePosixPath(packages[test["package"]])
            require(path.is_relative_to(package_path / "src") or path.is_relative_to(package_path / "tests/internal"), "internal test must be in src/ or tests/internal/")
        else:
            fields(semantics, ("category", "reason") if classification == "infrastructure" else ("reason",))
            text(semantics["reason"])
            if classification == "infrastructure":
                text(semantics["category"])
    unique(test_ids, "global test IDs")
    unique(identities, "Cargo tests")
    for contract in contracts.values():
        require(contract["id"] in asserted or contract["gap"] is not None, f"{contract['id']}: asserts evidence or explicit gap required")
    inventory_keys, inventoried = [], []
    for inventory in bundle["inventories"]:
        fields(inventory, ("package", "target", "tests"))
        require(text(inventory["package"]) in packages, "unknown inventory package")
        cargo_target(inventory["target"])
        names = strings(inventory["tests"])
        require(all(TEST_NAME.fullmatch(name) for name in names), "invalid inventory test name")
        unique(names, "inventory test identities")
        key = target_key(inventory)
        inventory_keys.append(key)
        inventoried.extend((*key, name) for name in names)
    unique(inventory_keys, "Cargo inventory targets")
    require(Counter(identities) == Counter(inventoried), "parsed tests and inventory declarations differ")
    excluded_keys = []
    for excluded in bundle["excluded_targets"]:
        fields(excluded, ("package", "target", "reason"))
        require(text(excluded["package"]) in packages, "unknown excluded package")
        cargo_target(excluded["target"])
        require(target_key(excluded) not in inventory_keys, "excluded target is inventoried")
        text(excluded["reason"])
        excluded_keys.append(target_key(excluded))
    unique(excluded_keys, "excluded target identities")
    for example in bundle["examples"]:
        fields(example, ("package", "example", "source", "kind", "reason"))
        require(text(example["package"]) in packages, "unknown example package")
        require(CARGO_NAME.fullmatch(text(example["example"])), "invalid Cargo example identity")
        require(example["kind"] in ("OpenApi", "Program"), "unsupported example kind")
        text(example["reason"])
        repository_file(root, example["source"])
        require(belongs(example["source"], example["package"]), "example source ownership mismatch")
    unique([(example["package"], example["example"]) for example in bundle["examples"]], "Cargo examples")
    return bundle


def directory_traces(root):
    repository = Path(__file__).resolve().parent.parent
    manifest = repository / "tools/traceability/Cargo.toml"
    args = ["cargo", "run", "--quiet", "--locked", "--manifest-path", str(manifest)]
    if "CARGO_TARGET_DIR" not in os.environ:
        args += ["--target-dir", str(repository / "target")]
    output = execute(args + ["--", str(root)], root)
    return json.loads(output, object_pairs_hook=no_duplicate_keys)


def load_traces(root):
    return validate_bundle(root, directory_traces(root))


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


def check_surfaces(bundle, root):
    openapi, grpc = {}, {}
    for component in bundle["components"]:
        for operation in component["operations"]:
            surface = operation["surface"]
            if surface["kind"] == "OpenApi":
                key = surface["package"], surface["example"]
                openapi.setdefault(key, {})[surface["operation"]] = operation["contract"]
            elif surface["kind"] == "Grpc":
                grpc[surface["path"], surface["service"], surface["operation"]] = operation["contract"]

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

def mapped_tests(bundle, kind="all"):
    allowed = {"boundary", "internal", "infrastructure"} if kind == "all" else {kind}
    tests = {}
    for test in bundle["tests"]:
        if test["classification"] in allowed:
            key = (*target_key(test), test["name"])
            if key not in tests:
                tests[key] = (test, [test["id"]])
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
    parser.add_argument("--kind", choices=("boundary", "internal", "infrastructure", "all"), default="all", help="test classification to run (all excludes helpers)")
    parser.add_argument("--output", type=Path, help="viewer HTML destination (viewer only)")
    args = parser.parse_args(argv)
    if args.output is not None and args.command != "viewer":
        parser.error("--output is only available for viewer")
    root = args.root.resolve()
    try:
        traces = load_traces(root)
        check_surfaces(traces, root)
        check_inventory([(test, [test["id"]]) for test in traces["tests"]], root, traces)
        tests = list(mapped_tests(traces, args.kind))
        if args.command == "viewer":
            from traceability_viewer import write_viewer

            destination = args.output if args.output is not None else root / "target/traceability-viewer.html"
            write_viewer(root, traces, destination)
            print(f"Viewer snapshot for {len(traces['features'])} features: {destination.resolve()}")
            return 0
        if args.command == "run":
            run_tests(tests, root)
        else:
            print(f"Traceability surfaces, structure, and Cargo identities match: {len(traces['features'])} features")
        return 0
    except (InvalidTrace, OSError, ValueError) as error:
        print(f"traceability: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
