#!/usr/bin/env python3
"""Check test layout and compare Rust test bodies across structural moves."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[1]
LEXEME = re.compile(r'[A-Za-z_][A-Za-z_0-9]*|[0-9][A-Za-z_0-9.]*|::|->|=>|&&|\|\||[^\s]')
CHAR = re.compile(r"b?'(?:\\(?:u\{[^}]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\n])'")
RAW = re.compile(r'(?:br|cr|r)(#*)"')


def tokens(source):
    result = []
    position = 0
    while position < len(source):
        if source[position].isspace():
            position += 1
        elif source.startswith('//', position):
            newline = source.find('\n', position)
            position = len(source) if newline < 0 else newline + 1
        elif source.startswith('/*', position):
            depth = 1
            position += 2
            while depth:
                if position >= len(source):
                    raise ValueError('unterminated Rust block comment')
                if source.startswith('/*', position):
                    depth += 1
                    position += 2
                elif source.startswith('*/', position):
                    depth -= 1
                    position += 2
                else:
                    position += 1
        else:
            start = position
            raw = RAW.match(source, position)
            char = CHAR.match(source, position)
            if raw:
                end = source.find('"' + raw[1], raw.end())
                if end < 0:
                    raise ValueError('unterminated Rust raw string')
                position = end + len(raw[1]) + 1
            elif char:
                position = char.end()
            elif source[position] == '"' or source.startswith(('b"', 'c"'), position):
                position += 1 if source[position] == '"' else 2
                while position < len(source) and source[position] != '"':
                    position += 2 if source[position] == '\\' else 1
                if position >= len(source):
                    raise ValueError('unterminated Rust string')
                position += 1
            else:
                position = LEXEME.match(source, position).end()
            result.append(source[start:position])
    return result


def balanced(values, start, opening, closing):
    depth = 0
    for index in range(start, len(values)):
        if values[index] == opening:
            depth += 1
        elif values[index] == closing:
            depth -= 1
            if not depth:
                return index + 1
    raise ValueError(f'unbalanced Rust {opening}{closing}')


def test_functions(values):
    index = 0
    while index < len(values):
        if values[index:index + 2] != ['#', '[']:
            index += 1
            continue
        start = index
        attributes = []
        while values[index:index + 2] == ['#', '[']:
            end = balanced(values, index + 1, '[', ']')
            attributes.append(values[index + 2:end - 1])
            index = end
        is_test = any(attribute[:1] == ['test'] or attribute[:3] == ['tokio', '::', 'test']
                      for attribute in attributes)
        if not is_test:
            continue
        cursor = index
        while cursor < len(values) and values[cursor] in ('async', 'unsafe', 'pub'):
            cursor += 1
        if values[cursor:cursor + 1] != ['fn']:
            raise ValueError('test attribute is not attached to a function')
        name = values[cursor + 1]
        body = values.index('{', cursor + 2)
        end = balanced(values, body, '{', '}')
        yield name, values[start:end]
        index = end


def files(root):
    process = subprocess.run(['git', '-C', str(root), 'ls-files', '-cz', '--others', '--exclude-standard'],
                             capture_output=True, check=True)
    return [root / path for path in sorted(set(process.stdout.decode().split('\0')))
            if path and (root / path).is_file()]


def package(path, root):
    for directory in path.parents:
        manifest = directory / 'Cargo.toml'
        if manifest.is_file():
            data = tomllib.loads(manifest.read_text())
            if 'package' in data:
                return data['package']['name']
        if directory == root:
            break
    return '@workspace'


def canonical_body(values, path):
    values = values.copy()
    for index in range(len(values) - 3):
        if values[index] in ('include_str', 'include_bytes') and values[index + 1:index + 3] == ['!', '(']:
            literal = values[index + 3]
            if literal.startswith('"'):
                fixture = path.parent / json.loads(literal)
                values[index + 3] = 'fixture-sha256:' + hashlib.sha256(fixture.read_bytes()).hexdigest()
    return hashlib.sha256(json.dumps(values, ensure_ascii=False).encode()).hexdigest()


def snapshot(root):
    tests = []
    fixtures = []
    interfaces = []
    for path in files(root):
        relative = path.relative_to(root)
        if path.suffix == '.rs':
            for name, body in test_functions(tokens(path.read_text())):
                tests.append({'package': package(path, root), 'name': name,
                              'body': canonical_body(body, path)})
            if relative.parts[:2] in (('tests', 'contracts'), ('tests', 'interface_checks')):
                interfaces.append(hashlib.sha256(path.read_bytes()).hexdigest())
        elif 'fixtures' in relative.parts or path.suffix == '.pem':
            fixtures.append(hashlib.sha256(path.read_bytes()).hexdigest())
    return {'version': 1, 'tests': sorted(tests, key=lambda test: (test['package'], test['name'], test['body'])),
            'fixtures': sorted(fixtures), 'interfaces': sorted(interfaces)}


def differences(before, after):
    if before.get('version') != 1 or not before.get('tests'):
        raise ValueError('baseline must contain a nonempty version-1 test snapshot')
    errors = []
    for category in ('tests', 'fixtures', 'interfaces'):
        old = Counter(json.dumps(item, sort_keys=True) for item in before[category])
        new = Counter(json.dumps(item, sort_keys=True) for item in after[category])
        if old != new:
            errors.append(f'{category}: missing {list((old - new).elements())}; added {list((new - old).elements())}')
    return errors


def layout_errors(root):
    errors = []
    for path in files(root):
        relative = path.relative_to(root)
        parts = relative.parts
        workspace_test = len(parts) > 3 and parts[0] in ('crates', 'apps') and 'tests' in parts[2:]
        if parts[:2] == ('tests', 'contracts') or (workspace_test and 'contracts' in parts):
            errors.append(f'{relative}: use feature/behavior test groups instead of contracts directories')
        if workspace_test:
            first_tests = parts.index('tests', 2)
            for directory in parts[first_tests + 1:-1]:
                if '-' in directory:
                    errors.append(f'{relative}: use snake_case test module directories')
            if parts[2] == 'tests' and len(parts) > 4 and path.name == 'tests.rs':
                errors.append(f'{relative}: name the scenario file after its behavior')
        if path.suffix != '.rs':
            continue
        values = tokens(path.read_text())
        support = path.stem == 'support' or 'support' in parts or path.stem.endswith('_support')
        if support and list(test_functions(values)):
            errors.append(f'{relative}: support contains test cases; load them from a suite or feature module')
        if support:
            children = []
            for index in range(len(values) - 2):
                if values[index] == 'mod' and values[index + 2] == ';':
                    directory = path.parent if path.name == 'mod.rs' else path.parent / path.stem
                    children.extend([directory / f'{values[index + 1]}.rs', directory / values[index + 1] / 'mod.rs'])
            for match in re.finditer(r'#\[path\s*=\s*"([^"]+)"\]\s*mod\s+\w+\s*;', path.read_text()):
                children.append(path.parent / match[1])
            for child in children:
                if child.is_file() and list(test_functions(tokens(child.read_text()))):
                    errors.append(f'{relative}: support loads test cases from {child.relative_to(root)}')
    return errors



def inventory(artifacts):
    result = []
    for line in artifacts.read_text().splitlines():
        artifact = json.loads(line)
        if artifact.get('reason') != 'compiler-artifact' or not artifact.get('executable') or not artifact['profile']['test']:
            continue
        manifest = Path(artifact['manifest_path'])
        package_name = tomllib.loads(manifest.read_text())['package']['name']
        def listing(*arguments):
            process = subprocess.run([artifact['executable'], '--list', '--format', 'terse', *arguments],
                                     cwd=manifest.parent, capture_output=True, text=True, check=True)
            return [name for name, kind in re.findall(r'^(.+): (test|benchmark)$', process.stdout, re.MULTILINE)]
        result.append({'package': package_name, 'kind': artifact['target']['kind'],
                       'target': artifact['target']['name'], 'tests': listing(), 'ignored': listing('--ignored')})
    if not result:
        raise ValueError('no compiled test executables found')
    return sorted(result, key=lambda target: (target['package'], target['kind'], target['target']))


def inventory_differences(before, after, renames):
    mapped = dict(rename.split('=', 1) for rename in renames)
    def normalized(records, rename):
        targets = []
        for target in records:
            key = f"{target['package']}:{target['target']}"
            name = mapped.get(key, target['target']) if rename else target['target']
            targets.append(json.dumps({'package': target['package'], 'kind': target['kind'], 'target': name,
                                       'tests': sorted(name.split('::')[-1] for name in target['tests']),
                                       'ignored': sorted(name.split('::')[-1] for name in target['ignored'])}, sort_keys=True))
        return Counter(targets)
    old, new = normalized(before, True), normalized(after, False)
    return [] if old == new else [f'compiled targets: missing {list((old - new).elements())}; added {list((new - old).elements())}']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', nargs='?', choices=('check', 'snapshot', 'compare', 'inventory', 'compare-inventory'), default='check')
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--baseline', type=Path)
    parser.add_argument('--artifacts', type=Path)
    parser.add_argument('--rename-target', action='append', default=[])
    args = parser.parse_args()
    root = args.root.resolve()
    if args.command in ('inventory', 'compare-inventory'):
        if args.artifacts is None:
            parser.error('inventory commands require --artifacts')
        result = inventory(args.artifacts)
        if args.command == 'inventory':
            if args.output is None:
                parser.error('inventory requires --output')
            args.output.write_text(json.dumps(result, indent=2) + '\n')
            print(f'Captured {sum(len(target["tests"]) for target in result)} tests in {len(result)} compiled targets')
            return 0
        if args.baseline is None:
            parser.error('compare-inventory requires --baseline')
        errors = inventory_differences(json.loads(args.baseline.read_text()), result, args.rename_target)
        if errors:
            print('\n'.join(errors), file=sys.stderr)
            return 1
        print('Compiled test inventory and ignored flags match the baseline')
        return 0
    if args.command == 'snapshot':
        if args.output is None:
            parser.error('snapshot requires --output')
        result = snapshot(root)
        if not result['tests']:
            parser.error('no Rust tests found')
        args.output.write_text(json.dumps(result, indent=2) + '\n')
        print(f'Captured {len(result["tests"])} test bodies, {len(result["fixtures"])} fixtures, {len(result["interfaces"])} interface inputs')
        return 0
    if args.command == 'compare':
        if args.baseline is None:
            parser.error('compare requires --baseline')
        errors = differences(json.loads(args.baseline.read_text()), snapshot(root))
    else:
        errors = layout_errors(root)
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        return 1
    print('Test layout check passed' if args.command == 'check' else 'Test bodies, attributes and fixture contents match the baseline')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
