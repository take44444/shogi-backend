#!/usr/bin/env python3
"""Compare collector --scan snapshots without depending on module or target names."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path


def bodies(snapshot):
    if isinstance(snapshot, dict) and snapshot.get("version") == 1 and "test_fingerprints" in snapshot:
        result = Counter((t.get("expected_package", t["package"]), t["function"], t["sha256"]) for t in snapshot["test_fingerprints"])
        if not result:
            raise ValueError("empty baseline cannot establish preservation")
        return result
    if not isinstance(snapshot, dict) or not isinstance(snapshot.get("tests"), list):
        raise ValueError("collector scan with tests is required")
    result = Counter()
    for test in snapshot["tests"]:
        tokens = test["function_tokens"]
        if not tokens or not test["body_source"]:
            raise ValueError("complete function tokens and body source are required")
        digest = hashlib.sha256(tokens.encode()).hexdigest()
        result[test["package"], test["function"], digest] += 1
    if not result:
        raise ValueError("empty scan cannot establish preservation")
    return result


def compare(before, after):
    old, new = bodies(before), bodies(after)
    missing, added = old - new, new - old
    if missing:
        raise ValueError(f"lost or changed original tests: {dict(missing)}")
    return sum(old.values()), sum(added.values())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    args = parser.parse_args()
    try:
        count, added = compare(json.loads(args.before.read_text()), json.loads(args.after.read_text()))
    except (KeyError, ValueError) as error:
        parser.exit(1, f"migration: {error}\n")
    print(f"Preserved {count} original test functions; {added} new tests.")


if __name__ == "__main__":
    main()
