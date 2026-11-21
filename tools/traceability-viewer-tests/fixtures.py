import subprocess
import sys
from pathlib import Path

REPO = Path(sys.argv[1])
DEST = Path(sys.argv[2])
DEST.mkdir(parents=True, exist_ok=True)
sys.path.insert(0, str(REPO / "scripts"))
import traceability_viewer as viewer


def make_fixture(name, components, apis, calls, tests, references=(), unresolved=()):
    root = DEST / name
    root.mkdir(exist_ok=True)
    files = {record["path"] for record in [*apis, *tests, *calls, *references, *unresolved]
             if not record["path"].startswith("@generated/")}
    for path in files:
        file = root / path
        file.parent.mkdir(parents=True, exist_ok=True)
        lines = [f"// {path} captured line {i}" for i in range(1, 41)]
        file.write_text("\n".join(lines) + '\n// </script><script>globalThis.sourceExecuted = true;</script>\n')
    units = [{"package": component, "target_kind": "lib", "target_name": component.replace("-", "_"),
              "is_test": False, "features": [], "purposes": ["production"], "source": f"src/{component}.rs"}
             for component in components]
    for unit in units:
        file = root / unit["source"]
        file.parent.mkdir(exist_ok=True)
        if not file.exists():
            file.write_text("// unit\n")
    bundle = {
        "version": 3,
        "components": [{"id": component, "package": component, "path": "."} for component in components],
        "apis": apis, "calls": calls, "tests": tests, "references": list(references),
        "unresolved": list(unresolved), "units": units,
        "excluded_targets": [{"package": components[0], "target_kind": "test", "target_name": "optional",
                              "reason": "requires unavailable feature", "required_features": ["optional-feature"]}]
                            if components else [],
        "coverage": {"bodies": 12},
        "configuration": {"scope": "Browser fixture / current host / default features",
                          "toolchain": "nightly-fixture", "cargo": ["build --workspace", "test --no-run"]},
        "generated_sources": {"@generated/domain/wire.rs": "pub fn generated_api() {}\n"},
    }
    if name == "graph":
        commands = [("init", "-q"), ("config", "user.name", "Viewer fixture"),
                    ("config", "user.email", "fixture@example.invalid"), ("add", "."),
                    ("commit", "-qm", "viewer fixture"),
                    ("remote", "add", "origin", "https://github.com/example/fixture.git")]
        for args in commands:
            subprocess.run(["git", "-C", str(root), *args], check=True, capture_output=True)
    viewer.write_viewer(root, bundle, DEST / f"{name}.html")


def api(identity, package, name, kind="function", line=2):
    return {"id": identity, "package": package, "name": name, "kind": kind,
            "path": f"src/{package}.rs", "line": line}


def test(name, package, targets, line=10):
    return {"name": name, "package": package, "target_kind": "test", "target_name": "contracts",
            "path": f"tests/{package}.rs", "line": line, "targets": targets}


def origin(package, production=True):
    return {"package": package, "target_kind": "lib", "target_name": package, "is_test": not production,
            "features": [], "purposes": ["production" if production else "tests"], "source": f"src/{package}.rs"}


def call(caller, callee, package, line=5, production=True):
    return {"caller": caller, "callee": callee, "path": f"src/{package}.rs", "line": line,
            "dispatch": "trait" if callee == "domain/trait" else "direct", "origins": [origin(package, production)]}


components = ["app", "domain", "store", "isolated", *[f"adapter-{i}" for i in range(6)]]
apis = [api("app/run", "app", "app::run"),
        api("app/internal", "app", "app::InternalPort::lookup", "trait_method"),
        api("domain/trait", "domain", "domain::Boundary::apply", "trait_method"),
        api("domain/impl", "domain", "domain::Engine::apply", "method"),
        api("domain/no-test", "domain", "domain::read"),
        api("store/run", "store", "store::run"),
        api("isolated/api", "isolated", "isolated::standalone")]
apis.append({"id": "domain/generated", "package": "domain", "name": "domain::generated_api",
             "kind": "function", "path": "@generated/domain/wire.rs", "line": 1})
apis += [api(f"adapter-{i}/run", f"adapter-{i}", f"adapter_{i}::run") for i in range(6)]
calls = [call("app/run", "domain/trait", "app"),
         call("app/run", "domain/trait", "app", line=6),
         call("app/run", "domain/no-test", "app", line=7),
         call("store/run", "domain/impl", "store", production=False),
         call("app/run", "app/internal", "app")]
calls += [call(f"adapter-{i}/run", "app/run", f"adapter-{i}") for i in range(6)]
tests = [test("declared_boundary_contract", "domain", ["domain/trait"]),
         test("concrete_implementation_only", "store", ["domain/impl"]),
         test("setup_only_untargeted", "app", []),
         test("app_run_contract", "app", ["app/run"])]
references = [{"caller": "store/run", "callee": "domain/trait", "path": "src/store.rs", "line": 8,
               "origins": [origin("store")]}]
unresolved = [{"caller": "store/run", "path": "src/store.rs", "line": 9,
               "reason": "function pointer unresolved", "origins": [origin("store")]}]
make_fixture("graph", components, apis, calls, tests, references, unresolved)
make_fixture("empty", [], [], [], [])
make_fixture("isolated", ["isolated"], [api("isolated/api", "isolated", "isolated::standalone")], [],
             [test("orphan_test", "isolated", [])])
make_fixture("bulk", ["bulk"], [api(f"bulk/{i:03}", "bulk", f"bulk::api_{i:03}") for i in range(100)], [],
             [test(f"bulk_test_{i:02}", "bulk", ["bulk/099"]) for i in range(20)])
print(DEST)
