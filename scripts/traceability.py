#!/usr/bin/env python3
"""Check source-backed feature graphs, surface ownership, and generated references."""
import argparse
from collections import Counter
import html
import json
import os
from pathlib import Path, PurePosixPath
import re
import shlex
import subprocess
import sys
import tempfile


DOCUMENTS = Path("docs/reference/traceability")
INDEX = Path("docs/reference/feature-traceability.md")
LEGACY = Path("docs/reference/ratings-traceability.md")
ID = re.compile(r"[a-z][a-z0-9-]*\Z")
CARGO_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_-]*\Z")
TEST_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*\Z")
RELATIONSHIP_LABELS = {"asserts": "事例を直接検証", "exercises": "処理を通る", "unreviewed": "所属のみ・未レビュー"}


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
    surfaces = {feature: [] for feature in features}

    def own(feature, label):
        require(text(feature) in features, f"{label}: unknown feature owner {feature}")
        surfaces[feature].append(label)

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
    require(all(surfaces.values()), f"orphan feature without surface or entrypoint: {sorted(feature for feature in features if not surfaces[feature])}")
    return surfaces


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


def escaped(value):
    return html.escape(value, quote=True).replace("|", "&#124;").replace("`", "&#96;")


def markdown(record, graph, lines, directory_bundle=None):
    def link(node):
        path = node["source"]["path"]
        return f"[{escaped(path)}](../../../{path}#L{lines[node['id']]})"

    guarantees = sorted(graph["guarantees"], key=lambda node: node["id"])
    contracts = sorted(graph["contracts"], key=lambda node: node["id"])
    tests = sorted(graph["tests"], key=lambda node: node["id"])
    feature = graph["feature"]
    out = [f"# {escaped(feature['title'])}", "",
           "このページは `python3 scripts/traceability.py render` で生成する。", "",
           f"元データは [{escaped(record.as_posix())}](../../../{record.as_posix()})。",
           "[全機能の一覧](../feature-traceability.md)。", "",
           "## 対象機能", "", "| ID | 機能 | 仕様 |", "| --- | --- | --- |",
           f"| {feature['id']} | {escaped(feature['title'])} | {link(feature)} |", "",
           "## 保証の成立条件と構成", ""]
    out += [f"{number}. {escaped(step)}" for number, step in enumerate(feature["flow"], 1)]
    out += ["", "以下の保証には、各行に示す契約が必要となる。テストによる検証は記載した事例の範囲に限られる。", "",
            "| ID | アプリケーションの保証と条件 | 必要な契約 |", "| --- | --- | --- |"]
    for guarantee in guarantees:
        refs = ", ".join(f"[{ref}](#contract-{ref})" for ref in guarantee["contracts"])
        out.append(f"| {guarantee['id']} | {escaped(guarantee['statement'])} | {refs} |")
    out += ["", "## 宣言した対応関係", "",
            "矢印は著者が宣言した関係を示す。「事例を直接検証」は記載した事例を直接確かめる関係、「処理を通る」は対象の処理を実行する関係を示す。「所属のみ・未レビュー」は契約ディレクトリへの配置だけで分類した関係を示す。", "",
            "```mermaid", "flowchart TD"]
    nodes = [feature] + guarantees + contracts + tests
    mermaid_ids = {node["id"]: f"n{index}" for index, node in enumerate(nodes)}
    for node in nodes:
        layer = "機能" if node is feature else "保証" if node in guarantees else "契約" if node in contracts else "テスト"
        label = escaped(f"{layer}: {node['id']}")
        out.append(f'    {mermaid_ids[node["id"]]}["{label}"]')
    for guarantee in guarantees:
        out.append(f"    {mermaid_ids[feature['id']]} --> {mermaid_ids[guarantee['id']]}")
        for ref in guarantee["contracts"]:
            out.append(f"    {mermaid_ids[guarantee['id']]} --> {mermaid_ids[ref]}")
    for contract in contracts:
        for edge in contract["evidence"]:
            out.append(f"    {mermaid_ids[contract['id']]} -->|{RELATIONSHIP_LABELS[edge['relationship']]}| {mermaid_ids[edge['test']]}")
    out += ["```", "", "## コンポーネント契約と根拠", ""]
    for contract in contracts:
        out += [f'<a id="contract-{contract["id"]}"></a>', "", f"### {contract['id']}", "",
                escaped(contract["statement"]), "", f"実装位置: {link(contract)}。", ""]
        for edge in contract["evidence"]:
            out.append(f"- [{edge['test']}](#test-{edge['test']}) ({RELATIONSHIP_LABELS[edge['relationship']]}): {escaped(edge['reason'])}")
        if "gap" in contract:
            out += ["", f"未検証: {escaped(contract['gap'])}"]
        out.append("")
    out += ["## 実在するテストと実行コマンド", "",
            "コマンドは個別テストを厳密一致で選択する。保存層テストには TEST_DATABASE_URL と TEST_VALKEY_URL が必要な場合がある。", ""]
    for test in tests:
        out += [f'<a id="test-{test["id"]}"></a>', "", f"### {test['id']}", "",
                f"テスト位置: {link(test)}。", "", f"範囲: {escaped(test['scope'])}", "", "```sh",
                shlex.join(cargo_command(test) + [test["name"], "--", "--exact", "--format", "pretty", "--color", "never"]), "```", ""]
    out += ["## 検査と根拠の限界", "",
            "check はグラフ、ファイル内の一意なテキスト位置、パッケージとターゲットごとのコンパイル済みテスト名、生成文書の一致を検査する。",
            "run は check の後に共有された Cargo テストを一度ずつ実行し、厳密一致の 1 件が成功し、失敗と無視が 0 件であることを検査する。",
            "この文書には実行済みの成功状態を保存しない。CI のワークスペース全体のテスト実行結果は、その実行時点の結果となる。",
            "位置の検査は Rust の構文解析ではなく、テスト名と指定したソース位置が同じ定義を指すことも証明しない。",
            "宣言した関連の意味と網羅性はレビューが必要となる。検出できる関連の削除は、必須の関連数、参照先、孤立ノードの規則に違反するものに限られる。", ""]
    out += [f"- {escaped(item)}" for item in graph["limitations"]]
    if directory_bundle:
        out += ["", "## ディレクトリ方式の検査範囲", "",
                "Rust 宣言、通常のモジュールと文字列リテラルの include! から物理ファイルの所属、パッケージ、ターゲット、完全なテスト名と位置を抽出する。ホスト環境で Cargo の既定の機能フラグを使い、ワークスペースのライブラリ、バイナリ、結合テストの全テスト一覧と一致させる。別のターゲット、機能フラグ、cfg の組合せは対象外。",
                "src/trace/<feature> と tests/trace/<feature> は新規機能も探索し、未宣言または未到達のファイルを拒否する。契約内の通常テストは「所属のみ・未レビュー」として追加する。「事例を直接検証」または「処理を通る」のレビュー済み根拠がない契約には未検証事項の gap 宣言が必要。",
                "全ターゲットの未分類テストは拒否する。trace ディレクトリ内の未対応の項目マクロと cfg は直接拒否し、外側の未対応構文が生成した実行対象テストはコンパイル済み一覧との差として拒否する。テスト実行補助と基盤の検査の明示分類および理由は全機能の一覧に示す。", ""]
        out = [line for line in out if not line.startswith("位置の検査は Rust の構文解析ではなく")]
    return "\n".join(out).rstrip() + "\n"


def index_markdown(traces, surfaces):
    out = ["# 機能の保証とテストの対応", "",
           "この一覧は `python3 scripts/traceability.py render` で生成する。", "",
           "実装を正とし、各機能の保証、コンポーネント契約、実在するテストと未検証事項を結ぶ。成功状態は保存しない。", "",
           "| 機能 | 公開操作と入口 | 保証 | 契約 | テスト宣言 | 元データ |",
           "| --- | --- | --- | --- | --- | --- |"]
    for record, graph, _ in traces:
        feature = graph["feature"]
        owned = "<br>".join(escaped(surface) for surface in sorted(surfaces[feature["id"]]))
        out.append(f"| [{escaped(feature['title'])}](traceability/{feature['id']}.md) | {owned} | {len(graph['guarantees'])} | {len(graph['contracts'])} | {len(graph['tests'])} | [{escaped(record.as_posix())}](../../{record.as_posix()}) |")
    out += ["", "## 一覧の検査範囲", "",
            "入口、保証、操作の所有者はコンパイルされる Feature、Contract、Case、Evidence、Surface 宣言から取得する。",
            "Cargo が列挙する全サンプルプログラムに、型検査される Example 宣言と理由を必須とする。OpenApi 宣言のサンプルだけを実行し、出力された操作と Surface 宣言の完全一致を検査する。Program 宣言はコンパイルのみ行う。proto/ の全サービス・RPC も探索して Surface 宣言と照合する。",
            "check は OpenAPI 出力プログラムと操作の追加・削除、gRPC 操作の追加・削除、未割当の所有者、台帳から参照されない機能、参照元データと生成文書の不足・余分を検出する。",
            "新しいバックグラウンド処理や WebSocket メッセージを自動発見すること、機能の分割が適切かを判定することはできない。台帳の入口とソース参照はレビューで保つ。",
            "テスト宣言数は機能内の件数であり、同じ Cargo テストが複数機能で共有されることがある。run はパッケージ・ターゲット・完全なテスト名が同じものを一度だけ実行する。",
            "「事例を直接検証」は記載した事例を直接確かめる関係、「処理を通る」は対象の処理を実行する関係を示す。位置の一致とテストの成功は、保証の意味や網羅性の証明ではない。", "",
            "更新方法は [仕様とテストの対応を更新する](../how-to/check-traceability.md) を参照する。", ""]
    bundle = traces.directory_bundle
    counts = Counter(item["classification"] for item in bundle["classifications"])
    out += ["## ワークスペース全体のテスト分類", "",
            f"コンパイル済みテスト {sum(counts.values())} 件。機能に関連付けた固有テスト {counts['evidence']} 件、基盤の検査 {counts['infrastructure']} 件、テスト実行補助 {counts['helper']} 件。共有テストは機能ごとの宣言数に重複して現れるが、この合計では一度だけ数える。", "",
            "標準ホストと Cargo の既定の機能フラグを使用する。test = false のターゲットは Cargo の指定どおり一覧から除く。required-features を持つターゲットは現在の規則では明示的に拒否する。テストがないターゲットも一覧照合の対象にする。", ""]
    for item in bundle["classifications"]:
        if item["classification"] == "evidence":
            continue
        identity, source, semantics = item["identity"], item["source"], item["semantics"]
        target = identity["target"]
        category = f" / {semantics['category']}" if "category" in semantics else ""
        label = f"{identity['package']} / {target['kind']} / {target.get('name', '(lib)')} / {identity['name']}"
        classification = {"infrastructure": "基盤の検査", "helper": "テスト実行補助"}[item['classification']]
        out.append(f"- `{escaped(label)}` ({classification}{category}) [{escaped(source['path'])}](../../{source['path']}#L{source['line']}): {escaped(semantics['reason'])}")
    for item in bundle["excluded_targets"]:
        out.append(f"- Cargo の除外ターゲット `{escaped(item['package'])} / {escaped(str(item['target']))}`: {escaped(item['reason'])}")
    out += ["", "## Cargo のサンプルプログラムの分類", ""]
    for item in bundle["examples"]:
        label = f"{item['package']} / {item['example']}"
        kind = {"OpenApi": "OpenAPI 仕様の出力", "Program": "プログラム（コンパイルのみ）"}[item['kind']]
        out.append(f"- `{escaped(label)}` ({kind}) [{escaped(item['source'])}](../../{item['source']}): {escaped(item['reason'])}")
    out.append("")
    return "\n".join(out)


def generated_documents(traces, surfaces):
    bundle = getattr(traces, "directory_bundle", {})
    migrated = {fragment["graph"]["feature"]["id"] for fragment in bundle.get("graphs", [])}
    documents = {DOCUMENTS / f"{graph['feature']['id']}.md": markdown(
        record, graph, lines, bundle if graph["feature"]["id"] in migrated else None)
        for record, graph, lines in traces}
    documents[INDEX] = index_markdown(traces, surfaces)
    return documents


def write_documents(root, documents):
    for path, content in documents.items():
        destination = root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=destination.parent, delete=False) as temporary:
            temporary.write(content)
            temporary_path = Path(temporary.name)
        try:
            os.replace(temporary_path, destination)
        finally:
            temporary_path.unlink(missing_ok=True)
    for path in existing_documents(root) - documents.keys():
        (root / path).unlink()


def existing_documents(root):
    return {path.relative_to(root) for path in (root / DOCUMENTS).rglob("*.md")} | {
        path for path in (INDEX, LEGACY) if (root / path).is_file()}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("check", "render", "run"))
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    args = parser.parse_args(argv)
    root = args.root.resolve()
    try:
        traces = load_traces(root)
        surfaces = check_surfaces(traces, root)
        tests = list(mapped_tests(traces))
        check_inventory(tests, root, traces.directory_bundle)
        documents = generated_documents(traces, surfaces)
        if args.command == "render":
            write_documents(root, documents)
            print(f"Rendered {len(documents)} documents for {len(traces)} features")
        else:
            require(existing_documents(root) == documents.keys(),
                    "generated document set mismatch: run python3 scripts/traceability.py render")
            for path, content in documents.items():
                require((root / path).read_text(encoding="utf-8") == content,
                        f"stale generated document: run python3 scripts/traceability.py render ({path})")
            if args.command == "run":
                run_tests(tests, root)
            else:
                print(f"Traceability surfaces, structure, Cargo identities, and documents match: {len(traces)} features")
        return 0
    except (InvalidTrace, OSError, ValueError) as error:
        print(f"traceability: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
