#!/usr/bin/env python3
from contextlib import ExitStack, redirect_stdout
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


LAUNCHER = Path(__file__).with_name("verify-app.py").resolve()
spec = importlib.util.spec_from_file_location("verify_app", LAUNCHER)
app = importlib.util.module_from_spec(spec)
spec.loader.exec_module(app)
RUN = subprocess.run
CHECK_OUTPUT = subprocess.check_output
POPEN = subprocess.Popen
RESOLVE = Path.resolve


class CheckoutTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="verify-checkout-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        (self.root / "scripts").mkdir()
        shutil.copyfile(LAUNCHER, self.root / "scripts/verify-app.py")
        (self.root / ".gitignore").write_text("/target/\n/.audit/\n__pycache__/\n")
        (self.root / "source.txt").write_text("committed source\n")
        self.git("init", "--quiet")
        self.git("config", "user.email", "verify@example.invalid")
        self.git("config", "user.name", "Verification test")
        self.git("config", "commit.gpgsign", "false")
        self.git("add", ".")
        self.git("commit", "--quiet", "-m", "Initial source")
        self.revision = self.git("rev-parse", "HEAD").strip()
        self.addCleanup(patch.stopall)
        patch.object(app, "ROOT", self.root).start()

    def git(self, *args):
        return RUN(["git", *args], cwd=self.root, check=True, text=True,
                   stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout

    def mutate(self, kind):
        if kind in ("staged", "unstaged"):
            (self.root / "source.txt").write_text("edited source\n")
            if kind == "staged":
                self.git("add", "source.txt")
        elif kind == "head":
            self.git("commit", "--quiet", "--allow-empty", "-m", "New HEAD")
        else:
            path = self.root / kind
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("new source\n")

    def cli(self, action, env=None):
        return RUN([sys.executable, str(self.root / "scripts/verify-app.py"), action],
                   cwd=self.root, env=env, text=True, capture_output=True)

    def test_dirty_cli_rejects_before_cargo_or_docker(self):
        tools = self.root / ".audit/bin"
        tools.mkdir(parents=True)
        invoked = self.root / ".audit/invoked"
        for name in ("cargo", "docker"):
            executable = tools / name
            executable.write_text(f"#!/bin/sh\nprintf '%s\\n' '{name}' >> '{invoked}'\nexit 93\n")
            executable.chmod(0o755)
        env = os.environ | {"PATH": str(tools) + os.pathsep + os.environ["PATH"]}
        for kind in ("staged", "unstaged", "new.txt", "nested/deep/new.txt"):
            self.mutate(kind)
            for action in ("play", "persistence", "records", "ratings", "gateway"):
                with self.subTest(kind=kind, action=action):
                    result = self.cli(action, env)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("clean checkout", result.stderr)
                    self.assertIn("source.txt" if kind in ("staged", "unstaged") else kind,
                                  result.stderr)
                    self.assertFalse(invoked.exists(), result.stdout + result.stderr)
            if kind in ("staged", "unstaged"):
                self.git("restore", "--staged", "--worktree", "source.txt")
            else:
                (self.root / kind).unlink()

    def lifecycle(self, feature, mutation=None, when="journey", cleanup_failure=False):
        calls, children = [], []
        processes = {}

        class Child:
            def __init__(child, args, **kwargs):
                child.pid = 90000 + len(children)
                child.running = True
                child.signals = []
                processes[child.pid] = Path(args[0])
                children.append(child)

            def poll(child):
                return None if child.running else 0

            def send_signal(child, value):
                child.signals.append(value)

            def wait(child, **kwargs):
                child.running = False
                return 0

        def resolve(path, *args, **kwargs):
            if str(path).startswith("/proc/") and path.name == "exe":
                return processes[int(path.parts[2])]
            return RESOLVE(path, *args, **kwargs)

        def output(args, **kwargs):
            if args[0] == "git":
                return CHECK_OUTPUT(args, **kwargs)
            if "config" in args:
                return json.dumps({"services": {"realtime-user-server": {"environment": {}}}})
            if args[:2] == ["docker", "compose"] and "ps" in args:
                project = args[args.index("-p") + 1]
                return json.dumps([{"Service": name, "State": "running", "Project": project}
                                   for name in ("postgres", "valkey", "game-server", "realtime-user-server",
                                                "realtime-gateway", "browser-edge", "outbox-publisher")])
            if "port" in args:
                if feature == "gateway":
                    return kwargs["env"]["REALTIME_PUBLIC_ORIGIN"].removeprefix("http://")
                return "127.0.0.1:54321\n"
            if args[:2] == ["docker", "ps"] or args[:3] in (
                    ["docker", "volume", "ls"], ["docker", "network", "ls"]):
                return ""
            raise AssertionError(f"unexpected external query: {args}")

        def command(directory, label, args, *unused, **kwargs):
            calls.append((label, args))
            (directory / (label + ".log")).write_text("external runtime completed\n")
            if (when == "journey" and (label.startswith("play-") or label == "browser")
                    or when == "cleanup" and label == "cleanup") and mutation:
                self.mutate(mutation)
            if label == "cleanup" and cleanup_failure:
                raise RuntimeError("owned project cleanup failed")

        def get(base, path):
            if path == "/openapi.json":
                body = {"paths": {"/game-records": {}, "/auth/google/start": {}}}
                status = 200
            elif path == "/game-records?from=0&to=1":
                body, status = {"items": []}, 200
            else:
                body, status = {}, 401
            return {"status": status, "body": json.dumps(body)}

        with ExitStack() as stack:
            stack.enter_context(patch.object(app, "command", command))
            stack.enter_context(patch.object(app, "get", get))
            stack.enter_context(patch.object(app, "start_token", return_value="123"))
            stack.enter_context(patch.object(app, "owns_listener", return_value=True))
            stack.enter_context(patch.object(Path, "resolve", resolve))
            stack.enter_context(patch.object(app.subprocess, "Popen", side_effect=lambda args, **kwargs:
                POPEN(args, **kwargs) if args[0] == "git" else Child(args, **kwargs)))
            stack.enter_context(patch.object(app.subprocess, "check_output", output))
            stack.enter_context(patch.object(app.subprocess, "run", side_effect=lambda args, **kwargs:
                RUN(args, **kwargs) if args[0] == "git" else
                subprocess.CompletedProcess(args, 0, "users" if feature == "gateway" else "shogi", "")))
            stack.enter_context(redirect_stdout(io.StringIO()))
            error = None
            try:
                app.gateway() if feature == "gateway" else app.run("play")
            except RuntimeError as caught:
                error = caught
        directory, = (self.root / "target/verification").iterdir()
        result = json.loads((directory / "result.json").read_text())
        state = json.loads((directory / "instance.json").read_text())
        self.assertEqual(state["revision"], self.revision)
        cleanup, = [args for label, args in calls if label == "cleanup"]
        self.assertEqual(cleanup[cleanup.index("-p") + 1], state["project"])
        self.assertIn("down", cleanup)
        self.assertIn("--volumes", cleanup)
        self.assertIn("--remove-orphans", cleanup)
        self.assertTrue((directory / "doctor.json").is_file())
        self.assertTrue(any(label == "browser" or label.startswith("play-") for label, _ in calls))
        for child in children:
            self.assertFalse(child.running)
            self.assertEqual(child.signals, [signal.SIGINT])
        return error, result

    def test_native_clean_ignored_outputs_pass(self):
        self.mutate("target/generated.rs")
        error, result = self.lifecycle("play")
        self.assertIsNone(error)
        self.assertEqual(result, {"feature": "play", "status": "passed", "cleanup_errors": []})

    def test_gateway_clean_ignored_outputs_pass(self):
        self.mutate("target/generated.rs")
        error, result = self.lifecycle("gateway")
        self.assertIsNone(error)
        self.assertEqual(result, {"feature": "gateway", "status": "passed", "cleanup_errors": []})

    def assert_final_rejected(self, feature, mutation, when):
        error, result = self.lifecycle(feature, mutation, when)
        self.assertIsNotNone(error)
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["cleanup_errors"], [])
        reason = "HEAD changed" if mutation == "head" else "clean checkout"
        self.assertIn(reason, str(error))
        self.assertIn(reason, result["error"])
        if mutation == "head":
            revision = self.git("rev-parse", "HEAD").strip()
            self.assertNotEqual(revision, self.revision)
            self.assertIn(self.revision, result["error"])
            self.assertIn(revision, result["error"])
        else:
            self.assertTrue(self.git("status", "--porcelain=v1", "--untracked-files=all"))

    def test_native_source_change_during_journey_fails(self):
        self.assert_final_rejected("play", "unstaged", "journey")

    def test_native_head_change_during_journey_fails(self):
        self.assert_final_rejected("play", "head", "journey")

    def test_gateway_source_change_during_journey_fails(self):
        self.assert_final_rejected("gateway", "nested/new.rs", "journey")

    def test_gateway_head_change_during_journey_fails(self):
        self.assert_final_rejected("gateway", "head", "journey")

    def test_native_source_change_during_cleanup_fails(self):
        self.assert_final_rejected("play", "staged", "cleanup")

    def test_gateway_source_change_during_cleanup_fails(self):
        self.assert_final_rejected("gateway", "unstaged", "cleanup")

    def test_native_head_change_during_cleanup_fails(self):
        self.assert_final_rejected("play", "head", "cleanup")

    def test_gateway_head_change_during_cleanup_fails(self):
        self.assert_final_rejected("gateway", "head", "cleanup")

    def test_native_cleanup_failure_still_fails(self):
        error, result = self.lifecycle("play", cleanup_failure=True)
        self.assertIsNotNone(error)
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["cleanup_errors"], ["owned project cleanup failed"])

    def test_gateway_cleanup_failure_still_fails(self):
        error, result = self.lifecycle("gateway", cleanup_failure=True)
        self.assertIsNotNone(error)
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["cleanup_errors"], ["owned project cleanup failed"])

    def test_doctor_remains_read_only_in_dirty_checkout(self):
        self.mutate("unstaged")
        directory = self.root / "target/verification/live"
        directory.mkdir(parents=True)
        state = {"root": str(self.root), "feature": "play", "revision": self.revision,
                 "processes": {}, "http": "http://127.0.0.1:54321"}
        app.save(directory, "instance.json", state)
        before = self.git("status", "--porcelain=v1", "--untracked-files=all")
        with patch.object(app, "get", side_effect=[
                {"status": 200, "body": '{"paths": {"/game-records": {}}}'},
                {"status": 200, "body": '{"items": []}'}]):
            self.assertTrue(app.doctor(directory)["healthy"])
        self.assertEqual(self.git("status", "--porcelain=v1", "--untracked-files=all"), before)
        self.assertEqual(list(directory.iterdir()), [directory / "instance.json"])


if __name__ == "__main__":
    unittest.main()
