#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('test_layout', Path(__file__).with_name('check-test-layout.py'))
layout = importlib.util.module_from_spec(spec)
spec.loader.exec_module(layout)


class LayoutTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        self.write('Cargo.toml', '[package]\nname = "fixture"\nversion = "0.1.0"\n')
        self.write('src/fixtures/data.sql', 'SELECT 1;\n')
        self.source = '#[traceability::targets(api::read)]\n#[tokio::test]\nasync fn behavior() { assert_eq!(include_str!("fixtures/data.sql"), "SELECT 1;\\n"); }\n'
        self.write('src/tests.rs', self.source)

    def write(self, path, content):
        destination = self.root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(content)
        return destination

    def test_moved_and_formatted_test_with_same_included_bytes_is_equivalent(self):
        before = layout.snapshot(self.root)
        (self.root / 'src/tests.rs').unlink()
        self.write('src/feature/scenario.rs', self.source.replace('fixtures/data.sql', '../fixtures/data.sql').replace('assert_eq!', '\n    assert_eq!'))
        self.assertEqual(layout.differences(before, layout.snapshot(self.root)), [])

    def test_changed_assertion_attribute_name_or_multiplicity_is_detected(self):
        before = layout.snapshot(self.root)
        for source in (self.source.replace('assert_eq!', 'assert_ne!'),
                       self.source.replace('#[tokio::test]', '#[tokio::test]\n#[ignore]'),
                       self.source.replace('behavior()', 'different()'),
                       self.source + self.source, ''):
            with self.subTest(source=source):
                self.write('src/tests.rs', source)
                self.assertTrue(layout.differences(before, layout.snapshot(self.root)))

    def test_changed_fixture_and_interface_input_are_detected(self):
        self.write('tests/interface_checks/valid.rs', 'fn valid() {}\n')
        before = layout.snapshot(self.root)
        self.write('src/fixtures/data.sql', 'SELECT 2;\n')
        self.assertTrue(layout.differences(before, layout.snapshot(self.root)))
        self.write('src/fixtures/data.sql', 'SELECT 1;\n')
        self.write('tests/interface_checks/valid.rs', 'fn invalid() {}\n')
        self.assertTrue(layout.differences(before, layout.snapshot(self.root)))

    def test_empty_or_missing_baseline_is_not_a_success(self):
        with self.assertRaises(ValueError):
            layout.differences({'version': 1, 'tests': []}, layout.snapshot(self.root))
        with self.assertRaises(ValueError):
            layout.differences({}, layout.snapshot(self.root))

    def test_rust_source_inside_strings_and_comments_is_not_a_test(self):
        source = 'const TEXT: &str = r##"#[test] fn fake() { \" }"##;\n/* nested /* #[test] */ comment */\n'
        self.assertEqual(list(layout.test_functions(layout.tokens(source))), [])
        self.assertEqual(len(list(layout.test_functions(layout.tokens(source + self.source)))), 1)

    def test_legacy_tree_generic_case_name_and_support_test_loading_are_rejected(self):
        self.write('crates/example/tests/contracts/feature/tests.rs', '#[test] fn legacy() {}')
        self.write('crates/example/tests/support.rs', '#[path = "cases.rs"] mod cases;')
        self.write('crates/example/tests/cases.rs', '#[test] fn behavior() {}')
        errors = '\n'.join(layout.layout_errors(self.root))
        self.assertIn('contracts directories', errors)
        self.assertIn('name the scenario', errors)
        self.assertIn('support loads test cases', errors)

    def test_support_cannot_load_cases_through_standard_modules(self):
        self.write('crates/example/tests/support/mod.rs', 'mod cases;')
        self.write('crates/example/tests/support/cases.rs', '#[test] fn misplaced() {}')
        self.assertIn('support loads test cases', '\n'.join(layout.layout_errors(self.root)))

    def test_support_with_only_helpers_is_allowed(self):
        self.write('crates/example/tests/support/server.rs', 'pub(super) fn start() {}')
        self.assertEqual(layout.layout_errors(self.root), [])

    def test_compiled_inventory_preserves_target_ownership_and_ignored_flags(self):
        before = [{'package': 'fixture', 'kind': ['test'], 'target': 'contracts',
                   'tests': ['old::behavior'], 'ignored': ['old::behavior']}]
        after = [{'package': 'fixture', 'kind': ['test'], 'target': 'http',
                  'tests': ['new::behavior'], 'ignored': ['new::behavior']}]
        self.assertEqual(layout.inventory_differences(before, after, ['fixture:contracts=http']), [])
        for changed in ({**after[0], 'tests': []}, {**after[0], 'ignored': []},
                        {**after[0], 'tests': ['new::behavior', 'duplicate::behavior']},
                        {**after[0], 'package': 'other'}):
            self.assertTrue(layout.inventory_differences(before, [changed], ['fixture:contracts=http']))


if __name__ == '__main__':
    unittest.main()
