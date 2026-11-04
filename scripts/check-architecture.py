#!/usr/bin/env python3
"""Check the source-derived inventory and local links in architecture.md."""

import argparse
from dataclasses import dataclass
import difflib
import fnmatch
import os
from pathlib import Path
import re
import sys

if sys.version_info < (3, 11):
    sys.exit("Architecture checker requires Python 3.11 or newer.")

import tomllib
from typing import Any
from urllib.parse import unquote, urlsplit


START = "<!-- architecture-inventory:start -->"
END = "<!-- architecture-inventory:end -->"
DOCUMENT = Path("docs/design/architecture.md")


@dataclass(frozen=True)
class Member:
    name: str
    manifest: Path
    dependencies: tuple[str, ...]


def read_required(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    if not text.strip():
        raise ValueError(f"Required input is empty: {path}")
    return text


def manifest(path: Path) -> dict[str, Any]:
    return tomllib.loads(read_required(path))


def package_name(path: Path) -> str:
    name = manifest(path).get("package", {}).get("name")
    if not isinstance(name, str) or not name:
        raise ValueError(f"Missing package.name: {path}")
    return name


def workspace_members(root: Path) -> list[Member]:
    cargo = manifest(root / "Cargo.toml")
    workspace = cargo["workspace"]
    paths: set[Path] = set()
    for pattern in workspace["members"]:
        matches = list(root.glob(pattern))
        if not matches and not any(character in pattern for character in "*?["):
            raise ValueError(f"Workspace member pattern matches no paths: {pattern}")
        for path in matches:
            relative = path.relative_to(root).as_posix()
            if not any(fnmatch.fnmatchcase(relative, p) for p in workspace.get("exclude", [])):
                paths.add(path.resolve() / "Cargo.toml")
    if "package" in cargo:
        paths.add(root / "Cargo.toml")
    if not paths:
        raise ValueError("Workspace has no members")

    members = []
    for path in sorted(paths):
        data = manifest(path)
        sections = [data, *data.get("target", {}).values()]
        dependencies = set()
        for section in sections:
            for kind in ("dependencies", "build-dependencies"):
                for alias, spec in section.get(kind, {}).items():
                    base = path.parent
                    if isinstance(spec, dict) and spec.get("workspace") is True:
                        spec = workspace["dependencies"][alias]
                        base = root
                    if not isinstance(spec, dict) or "path" not in spec:
                        continue
                    dependency = (base / spec["path"] / "Cargo.toml").resolve()
                    if dependency.is_relative_to(root):
                        dependencies.add(package_name(dependency))
        members.append(Member(package_name(path), path, tuple(sorted(dependencies))))
    if len({member.name for member in members}) != len(members):
        raise ValueError("Workspace has duplicate package names")
    return sorted(members, key=lambda member: member.name)


def user_routes(root: Path) -> list[str]:
    path = root / "crates/user-http/src/lib.rs"
    source = read_required(path)
    message = f"Cannot parse every HTTP route in {path}; update the checker"
    # This deliberately accepts only the two supported registration forms. New
    # composition syntax must extend this parser and its command-line fixtures.
    if re.search(r"\.(?:nest|merge|route_service|nest_service|fallback|fallback_service)\s*\(", source):
        raise ValueError(message)
    classic_calls = re.findall(r"\.route\s*\(", source)
    classic = re.findall(r'\.route\s*\(\s*"([^"\\]+)"\s*,', source)
    registry_calls = re.findall(r"\.routes\s*\(", source)
    registrations = re.findall(r"\.routes\s*\(\s*routes!\(\s*([A-Za-z_][A-Za-z_0-9]*)\s*\)\s*\)", source)
    annotations = re.findall(
        r'#\[utoipa::path\(\s*(get|post|put|patch|delete|head|options|trace)\s*,\s*path\s*=\s*"([^"\\]+)"(.*?)\)\]\s*async\s+fn\s+([A-Za-z_][A-Za-z_0-9]*)\s*\(',
        source, re.DOTALL,
    )
    annotation_calls = re.findall(r"#\[utoipa::path\s*\(", source)
    macro_calls = re.findall(r"\broutes!\s*\(", source)
    if (len(classic_calls) != len(classic)
            or len(registry_calls) != len(registrations)
            or len(macro_calls) != len(registrations)
            or len(annotation_calls) != len(annotations)
            or (classic and registry_calls)):
        raise ValueError(message)
    handlers = [handler for _, _, _, handler in annotations]
    operations = [(method, route) for method, route, _, _ in annotations]
    if (len(set(registrations)) != len(registrations)
            or len(set(handlers)) != len(handlers)
            or len(set(operations)) != len(operations)
            or set(registrations) != set(handlers)
            or any(re.search(r"\b(?:context_path|path)\s*=", options) for _, _, options, _ in annotations)):
        raise ValueError(message)
    routes = classic + [route for _, route, _, _ in annotations]
    if not routes or any(not route.startswith("/") for route in routes):
        raise ValueError(message)
    return sorted(set(routes))


def user_tables(root: Path) -> list[str]:
    path = root / "crates/user-persistence/src/lib.rs"
    source = re.split(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]", read_required(path), maxsplit=1)[0]
    declarations = re.findall(r"\bCREATE\s+TABLE\b", source, re.IGNORECASE)
    tables = re.findall(
        r"\bCREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?([A-Za-z_][A-Za-z_0-9]*)\s*\(",
        source,
        re.IGNORECASE,
    )
    if not tables or len(declarations) != len(tables):
        raise ValueError(f"Cannot parse every CREATE TABLE name before #[cfg(test)] in {path}; update the checker")
    return sorted(set(tables))


def render_inventory(root: Path) -> str:
    lines = [
        "",
        "Cargoマニフェストとユーザーサービスのソースから生成。更新は `python3 scripts/check-architecture.py --write` を実行する。",
        "",
        "| Workspaceパッケージ | マニフェスト | 本番の内部依存 |",
        "|---|---|---|",
    ]
    for member in workspace_members(root):
        relative = member.manifest.relative_to(root).as_posix()
        link = os.path.relpath(member.manifest, root / DOCUMENT.parent)
        dependencies = ", ".join(f"`{name}`" for name in member.dependencies) or "なし"
        lines.append(f"| `{member.name}` | [{relative}]({link}) | {dependencies} |")
    lines += [
        "",
        "ユーザーHTTPのルートパス。定義は `crates/user-http/src/lib.rs`。",
        "",
        *[f"- `{route}`" for route in user_routes(root)],
        "",
        "ユーザーPostgreSQLのテーブル。定義は `crates/user-persistence/src/lib.rs`（テストコードを除く）。",
        "",
        *[f"- `{table}`" for table in user_tables(root)],
        "",
    ]
    return "\n".join(lines)


def inventory_bounds(document: str) -> tuple[int, int]:
    if document.count(START) != 1 or document.count(END) != 1:
        raise ValueError(f"{DOCUMENT} must contain exactly one {START} and one {END}")
    start = document.index(START) + len(START)
    end = document.index(END)
    if start > end:
        raise ValueError(f"Inventory markers are reversed in {DOCUMENT}")
    return start, end


def validate_links(root: Path, document: str) -> None:
    broken = []
    for match in re.finditer(r'!?\[[^\]\n]*\]\(\s*(?:<([^>]+)>|([^\s)]+))(?:\s+"[^"]*")?\s*\)', document):
        target = match.group(1) or match.group(2)
        url = urlsplit(target)
        if url.scheme or url.netloc or not url.path:
            continue
        path = unquote(url.path)
        destination = root / path.lstrip("/") if path.startswith("/") else root / DOCUMENT.parent / path
        if not destination.exists():
            broken.append(target)
    if broken:
        raise ValueError(f"Broken local links in {DOCUMENT}: " + ", ".join(broken))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write", action="store_true", help="refresh only the generated inventory block")
    mode.add_argument("--check", action="store_true", help="check without writing (default)")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    try:
        path = root / DOCUMENT
        document = read_required(path)
        start, end = inventory_bounds(document)
        validate_links(root, document[:start] + document[end:])
        expected = document[:start] + render_inventory(root) + document[end:]
        validate_links(root, expected)
        if expected == document:
            print("Architecture inventory and local links are current.")
            return 0
        if args.write:
            path.write_text(expected, encoding="utf-8")
            print(f"Updated the generated inventory in {DOCUMENT}.")
            return 0
        print("Architecture inventory is stale. Run: python3 scripts/check-architecture.py --write", file=sys.stderr)
        sys.stderr.writelines(difflib.unified_diff(
            document.splitlines(keepends=True), expected.splitlines(keepends=True),
            fromfile=str(DOCUMENT), tofile=f"{DOCUMENT} (generated)",
        ))
        return 1
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"Architecture check failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
