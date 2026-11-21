#!/usr/bin/env python3
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


CHECKER = Path(__file__).with_name("check-contract-interfaces.py").resolve()
spec = importlib.util.spec_from_file_location("contract_checker", CHECKER)
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class ContractInterfaceTests(unittest.TestCase):
    def test_cargo_artifact_layouts(self):
        rustc = os.environ.get("RUSTC", "rustc")
        run = subprocess.run
        for layout, metadata_first in [("deps", None), ("build", None), ("build", True), ("build", False)]:
            with self.subTest(layout=layout, metadata_first=metadata_first), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                artifacts = []

                def compile_crate(name, source, dependencies=(), metadata_only=False):
                    directory = root / "debug" / "deps" if layout == "deps" else root / "debug/build" / name / "hash/out"
                    directory.mkdir(parents=True, exist_ok=True)
                    source_file = root / f"{name}.rs"
                    source_file.write_text(source)
                    command = [rustc, "--edition=2024", "--crate-type=lib", "--crate-name", name,
                               "--emit=metadata" if metadata_only else "--emit=link,metadata",
                               "--out-dir", str(directory), str(source_file)]
                    for dependency, filename in dependencies:
                        command.extend(["--extern", f"{dependency}={filename}"])
                    result = run(command, text=True, capture_output=True)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    metadata = directory / f"lib{name}.rmeta"
                    library = directory / f"lib{name}.rlib"
                    filenames = [str(metadata)] if metadata_only else [str(library)]
                    if metadata_first is not None and not metadata_only:
                        result = run(["ar", "d", str(library), "lib.rmeta"], text=True, capture_output=True)
                        self.assertEqual(result.returncode, 0, result.stderr)
                        filenames = [str(metadata), str(library)] if metadata_first else [str(library), str(metadata)]
                    artifacts.append({"reason": "compiler-artifact", "target": {"name": name}, "filenames": filenames})
                    return metadata

                support = compile_crate("support", "pub struct Value { private: () }", metadata_only=True)
                for package in checker.PACKAGES:
                    compile_crate(package.replace("-", "_"), "pub use support::Value;", [("support", support)])

                contracts = root / "tests/contracts"
                contracts.mkdir(parents=True)
                (contracts / "valid.rs").write_text("\n".join(
                    f"pub fn {package.replace('-', '_')}(value: {package.replace('-', '_')}::Value) {{}}"
                    for package in checker.PACKAGES
                ))
                (contracts / "private_metadata.rs").write_text(
                    "pub fn invalid(value: game_application::Value) { let _ = value.private; }"
                )

                def cargo_output(command, *args, **kwargs):
                    if command[:2] == ["cargo", "build"]:
                        return subprocess.CompletedProcess(command, 0, "\n".join(map(json.dumps, artifacts)), "")
                    return run(command, *args, **kwargs)

                with patch.object(checker, "ROOT", root), patch.object(checker, "CASES", {"valid": None, "private_metadata": "E0616"}), patch.object(checker.subprocess, "run", side_effect=cargo_output):
                    checker.main()


if __name__ == "__main__":
    unittest.main()
