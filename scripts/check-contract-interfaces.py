#!/usr/bin/env python3
import json
import os
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
PACKAGES = ["game-application", "game-record-application", "game-record-persistence"]
CASES = {
    "valid": None,
    "private_participants": "E0616",
    "private_metadata": "E0616",
    "distinct_counts": "E0308",
    "no_writer": "E0432",
    "no_accounts": "E0599",
    "frozen_events": "E0616",
}


def main():
    command = ["cargo", "build", "--locked", "--offline", "--message-format=json"]
    for package in PACKAGES:
        command.extend(["-p", package])
    built = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
    if built.returncode:
        raise SystemExit(built.stdout + built.stderr)
    libraries = {}
    for line in built.stdout.splitlines():
        artifact = json.loads(line)
        if artifact.get("reason") != "compiler-artifact":
            continue
        name = artifact["target"]["name"]
        for filename in artifact["filenames"]:
            if filename.endswith(".rlib") and name.replace("_", "-") in PACKAGES:
                libraries[name] = Path(filename)
    assert set(libraries) == {package.replace("-", "_") for package in PACKAGES}, libraries
    with tempfile.TemporaryDirectory(prefix="shogi-contract-interfaces-") as output:
        for case, expected in CASES.items():
            command = [os.environ.get("RUSTC", "rustc"), "--edition=2024", "--crate-name", "contract",
                       "--crate-type=lib", "--error-format=json", "-A", "dead_code",
                       "--out-dir", output, str(ROOT / "tests/contracts" / f"{case}.rs")]
            for name, library in libraries.items():
                command.extend(["--extern", f"{name}={library}"])
            library_dir = next(iter(libraries.values())).parent
            dependency_dir = library_dir if library_dir.name == "deps" else library_dir / "deps"
            command.extend(["-L", f"dependency={dependency_dir}"])
            result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
            if expected is None:
                assert result.returncode == 0, result.stderr
            else:
                diagnostics = [json.loads(line) for line in result.stderr.splitlines() if line.startswith("{")]
                codes = {item["code"]["code"] for item in diagnostics if item.get("level") == "error" and item.get("code")}
                assert result.returncode != 0 and expected in codes, f"{case}: expected {expected}, got {result.returncode}\n{result.stderr}"
            print(f"{case}: passed")


if __name__ == "__main__":
    main()
