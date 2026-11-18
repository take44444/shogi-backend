#!/usr/bin/env python3
import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("migration", Path(__file__).with_name("check-traceability-migration.py"))
migration = importlib.util.module_from_spec(spec)
spec.loader.exec_module(migration)


class Preservation(unittest.TestCase):
    def setUp(self):
        self.before = {"tests": [dict(package="domain", function="rejects", function_tokens="fn rejects() { assert!(false); }", body_source="{ assert!(false); }")]}

    def test_target_and_module_moves_preserve_the_test(self):
        moved = copy.deepcopy(self.before)
        moved["tests"][0].update(target={"kind": "test", "name": "contracts"}, name="boundary::rejects", file="tests/contracts/rules.rs")
        self.assertEqual(migration.compare(self.before, moved), (1, 0))

    def test_same_count_with_changed_body_is_rejected(self):
        changed = copy.deepcopy(self.before)
        changed["tests"][0]["function_tokens"] = "fn rejects() { assert!(true); }"
        with self.assertRaisesRegex(ValueError, "lost or changed original tests"):
            migration.compare(self.before, changed)

    def test_optional_call_comma_formats_identically_but_tuple_change_does_not(self):
        self.assertEqual(migration.fingerprint("fn x() { f(1,); }"), migration.fingerprint("fn x() { f(1); }"))
        self.assertNotEqual(migration.fingerprint("fn x() { f((1,)); }"), migration.fingerprint("fn x() { f((1)); }"))

    def test_reassignment_to_another_package_is_rejected(self):
        changed = copy.deepcopy(self.before)
        changed["tests"][0]["package"] = "transport"
        with self.assertRaisesRegex(ValueError, "lost or changed original tests"):
            migration.compare(self.before, changed)

    def test_recorded_component_owner_move_preserves_the_original_body(self):
        package, function, digest = next(iter(migration.bodies(self.before)))
        baseline = {"version": 1, "test_fingerprints": [dict(package=package, function=function, sha256=digest, expected_package="application")]}
        changed = copy.deepcopy(self.before)
        changed["tests"][0]["package"] = "application"
        self.assertEqual(migration.compare(baseline, changed), (1, 0))

    def test_missing_or_empty_input_is_not_a_pass(self):
        for value in ({}, {"tests": []}, {"version": 1, "test_fingerprints": []}):
            with self.subTest(value=value), self.assertRaises(ValueError):
                migration.compare(value, self.before)

    def test_additions_are_reported(self):
        changed = copy.deepcopy(self.before)
        changed["tests"].append(dict(changed["tests"][0], function="new_case"))
        self.assertEqual(migration.compare(self.before, changed), (1, 1))


if __name__ == "__main__":
    unittest.main()
