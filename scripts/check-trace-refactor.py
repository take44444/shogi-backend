#!/usr/bin/env python3
"""Compare test bodies and trace relationships with a Git revision after a layout refactor."""

import argparse
from collections import Counter
import copy
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parent.parent


def require(condition, message):
    if not condition:
        raise ValueError(message)


def target_key(test):
    target = test["target"]
    return test["package"], target["kind"], target.get("name", "")


def test_key(test):
    return (*target_key(test), test["name"].split("::")[-1])


def collect(root):
    command = ["cargo", "run", "--quiet", "--locked", "--manifest-path",
               str(ROOT / "tools/traceability/Cargo.toml"), "--target-dir",
               str(Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve()), "--"]

    def run(*args):
        return json.loads(subprocess.check_output(command + list(args) + [str(root)], cwd=ROOT))

    scan, bundle = run("--scan"), run()
    tests = {}
    for test in scan["tests"]:
        key = test_key(test)
        require(key not in tests, f"ambiguous test function within target: {key}")
        tokens = test["function_tokens"]
        if test["function"] == "code_exchange_sends_pkce_and_verifies_signed_token_over_http":
            tokens = tokens.replace("super :: super :: now", "crate :: now")
        if test["function"] == "publisher_session_lock_excludes_a_second_process_and_releases_on_drop":
            tokens = tokens.replace('"publisher_lock_probe"', '"outbox::publisher_lock_probe"')
        tests[key] = {"tokens": tokens, "classification": test["classification"]}
        if "/trace/" in test["file"] and test["classification"] not in ("helper", "infrastructure"):
            tests[key]["owner"] = test["file"].split("/trace/", 1)[1].split("/")[:2]

    graphs = {}
    for fragment in bundle["graphs"]:
        graph = copy.deepcopy(fragment["graph"])
        ids = {test["id"]: test_key(test) for test in graph["tests"]}
        for test in graph["tests"]:
            test["id"] = test_key(test)
            test.pop("name")
            test.pop("source")
        graph["tests"].sort(key=lambda test: test["id"])
        for contract in graph["contracts"]:
            for edge in contract["evidence"]:
                edge["test"] = ids[edge["test"]]
            contract["evidence"].sort(key=lambda edge: edge["test"])
        graphs[graph["feature"]["id"]] = graph
    return {"tests": tests, "graphs": graphs,
            "targets": Counter(target_key(target) for target in scan["targets"]),
            "catalog": {key: bundle[key] for key in
                        ("entrypoints", "surfaces", "excluded_targets", "packages", "examples")}}


def compare(before, after, allowed_body_changes=()):
    require(before["targets"] == after["targets"], "Cargo targets changed")
    require(before["tests"].keys() == after["tests"].keys(), "test functions changed")
    changed = set()
    for key, test in before["tests"].items():
        current = after["tests"][key]
        if test["tokens"] != current["tokens"] and "/".join(key) in allowed_body_changes:
            changed.add("/".join(key))
            current = {**current, "tokens": test["tokens"]}
        require(test == current, f"test body, classification, or owner changed: {key}")
    require(changed == set(allowed_body_changes), "allowed test body changes must name exactly the changed tests")
    require(before["graphs"] == after["graphs"], "feature/contract/evidence relationships changed")
    require(before["catalog"] == after["catalog"], "entrypoints, interfaces, examples, or packages changed")


def controls(baseline):
    first = next(iter(baseline["tests"]))
    for change, expected in (("tokens", "test body"), ("owner", "test body")):
        changed = copy.deepcopy(baseline)
        changed["tests"][first][change] = "deliberate mismatch"
        try:
            compare(baseline, changed)
        except ValueError as error:
            require(expected in str(error), f"control failed for an unrelated reason: {error}")
        else:
            raise ValueError(f"comparison missed {change} mismatch")
    changed = copy.deepcopy(baseline)
    graph = next(iter(changed["graphs"].values()))
    graph["contracts"][0]["evidence"][0]["reason"] = "deliberate mismatch"
    try:
        compare(baseline, changed)
    except ValueError as error:
        require("relationships changed" in str(error), str(error))
    else:
        raise ValueError("comparison missed evidence mismatch")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True, help="Git revision before the layout refactor")
    parser.add_argument("--allow-test-body-change", action="append", default=[],
                        help="Explicit package/kind/target/function exception for an intentional test extension")
    args = parser.parse_args()
    revision = subprocess.check_output(["git", "rev-parse", "--verify", args.base + "^{commit}"],
                                       cwd=ROOT, text=True).strip()
    archive = subprocess.check_output(["git", "archive", revision], cwd=ROOT)
    with tempfile.TemporaryDirectory(prefix="trace-refactor-") as directory:
        base = Path(directory)
        with tarfile.open(fileobj=io.BytesIO(archive)) as files:
            files.extractall(base, filter="data")
        before = collect(base)
    controls(before)
    after = collect(ROOT)
    compare(before, after, args.allow_test_body_change)
    print(f"Preserved {len(after['tests']) - len(args.allow_test_body_change)} test bodies, "
          f"all {len(after['tests'])} classifications/owners, "
          f"{len(after['targets'])} Cargo targets, and {len(after['graphs'])} feature graphs. "
          f"Allowed test extensions: {args.allow_test_body_change}. "
          "Body, owner, and evidence mismatch controls passed.")


if __name__ == "__main__":
    main()
