#!/usr/bin/env python3
"""Run a disposable shogi API journey; retain evidence under target/verification."""
import argparse
import json
import os
from pathlib import Path
import re
import signal
import socket
import subprocess
import time
import urllib.error
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[1]


def require(ok, message):
    if not ok:
        raise RuntimeError(message)


def save(directory, name, value):
    (directory / name).write_text(json.dumps(value, indent=2) + "\n")


def command(directory, label, args, env=None, timeout=180):
    with (directory / f"{label}.log").open("a") as log:
        log.write(f"$ {args!r}\n")
        log.flush()
        result = subprocess.run(args, cwd=ROOT, env=env, stdout=log,
                                stderr=subprocess.STDOUT, timeout=timeout)
        log.write(f"exit={result.returncode}\n")
    require(result.returncode == 0, f"{label} failed; read {directory / (label + '.log')}")


def wait_for(check, description, timeout=60):
    deadline = time.monotonic() + timeout
    while not check():
        require(time.monotonic() < deadline, f"timed out: {description}")
        time.sleep(0.2)


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def start_token(pid):
    return Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[19]


def owns_listener(pid, port):
    inodes = set()
    for fd in Path(f"/proc/{pid}/fd").iterdir():
        try:
            inodes.add(os.readlink(fd))
        except FileNotFoundError:
            pass
    for line in Path(f"/proc/{pid}/net/tcp").read_text().splitlines()[1:]:
        fields = line.split()
        if (fields[1] == f"0100007F:{port:04X}" and fields[3] == "0A"
                and f"socket:[{fields[9]}]" in inodes):
            return True
    return False


def get(base, path):
    try:
        response = urllib.request.urlopen(base + path, timeout=5)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        return {"method": "GET", "url": base + path, "status": response.status,
                "headers": dict(response.headers), "body": response.read().decode()}


def http(directory, label, base, path, expected=200):
    response = get(base, path)
    save(directory, label + ".json", response)
    require(response["status"] == expected, f"{path}: expected {expected}, got {response['status']}")
    return json.loads(response["body"])


def doctor(directory):
    state = json.loads((directory / "instance.json").read_text())
    require(state["root"] == str(ROOT), "instance belongs to another checkout")
    for name, process in state["processes"].items():
        pid = process["pid"]
        require(start_token(pid) == process["start_token"], f"{name}: PID was reused")
        require(str(Path(f"/proc/{pid}/exe").resolve()) == process["exe"], f"{name}: wrong executable")
        if "port" in process:
            require(owns_listener(pid, process["port"]), f"{name}: does not own its port")
    schema = get(state["http"], "/openapi.json")
    require(schema["status"] == 200 and "/game-records" in json.loads(schema["body"])["paths"],
            "unexpected public API")
    response = get(state["http"], "/game-records?from=0&to=1")
    require(response["status"] == 200 and json.loads(response["body"])["items"] == [],
            "public database read failed")
    return {"healthy": True, **state}


def run(feature):
    run_id = "shogi-verify-" + uuid.uuid4().hex[:12]
    directory = ROOT / "target/verification" / run_id
    directory.mkdir(parents=True)
    print(f"Evidence: {directory}", flush=True)
    env = os.environ.copy()
    env.update(POSTGRES_PORT="0", COMPOSE_PROFILES="")
    compose = ["docker", "compose", "--env-file", "/dev/null", "-f", str(ROOT / "compose.yaml"), "-p", run_id]
    processes = []
    state = {"root": str(ROOT), "project": run_id, "feature": feature,
             "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
             "processes": {}}
    result = {"feature": feature, "status": "failed"}

    def start(name, process_env, port=None):
        executable = ROOT / "target/debug" / name
        with (directory / f"{name}.log").open("a") as log:
            child = subprocess.Popen([str(executable)], cwd=ROOT, env=process_env,
                                     stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        processes.append(child)
        record = {"pid": child.pid, "start_token": start_token(child.pid), "exe": str(executable)}
        if port is not None:
            record["port"] = port
        state["processes"][name] = record
        save(directory, "instance.json", state)
        if port is not None:
            def ready():
                require(child.poll() is None, f"{name} exited; read its log")
                return owns_listener(child.pid, port)
            wait_for(ready, name + " schema initialization and listener")
        return child

    def example(name, *args):
        command(directory, name + "-" + str(time.time_ns()),
                [str(ROOT / "target/debug/examples" / name), *args], timeout=90)

    try:
        command(directory, "build", ["cargo", "build", "--locked", "--target-dir", str(ROOT / "target"),
                                    "-p", "game-server", "-p", "player-server", "-p", "rating-worker-server",
                                    "-p", "game-grpc", "--bins", "--examples"], timeout=1200)
        command(directory, "postgres-launch", compose + ["up", "-d", "--wait", "postgres"], env)
        # Init uses a socket-only temporary server. Require final TCP login as shogi.
        def database_ready():
            check = subprocess.run(compose + ["exec", "-T", "-e", "PGPASSWORD=shogi", "postgres",
                                   "psql", "-h", "127.0.0.1", "-U", "shogi", "-d", "shogi",
                                   "-Atc", "SELECT current_database()"], env=env,
                                   capture_output=True, text=True, timeout=10)
            return check.returncode == 0 and check.stdout.strip() == "shogi"
        wait_for(database_ready, "init SQL and application TCP login")
        address = subprocess.check_output(compose + ["port", "postgres", "5432"], env=env, text=True).strip()
        db_port = int(address.rsplit(":", 1)[1])
        game_port, player_port = free_port(), free_port()
        state.update(grpc=f"http://127.0.0.1:{game_port}", http=f"http://127.0.0.1:{player_port}")
        app_env = env | {"DATABASE_URL": f"postgres://shogi:shogi@127.0.0.1:{db_port}/shogi",
                         "GAME_SERVER_ADDR": f"127.0.0.1:{game_port}",
                         "PLAYER_GAME_DATABASE_URL": f"postgres://shogi_reader:shogi_reader@127.0.0.1:{db_port}/shogi",
                         "PLAYER_BIND_ADDR": f"127.0.0.1:{player_port}"}
        game = start("game-server", app_env, game_port)
        start("player-server", app_env, player_port)
        save(directory, "worktree.json", {"diff": subprocess.check_output(["git", "diff", "HEAD"], cwd=ROOT, text=True)})
        save(directory, "doctor.json", doctor(directory))
        grpc, base = state["grpc"], state["http"]
        if feature == "play":
            example("play", grpc, run_id)
        elif feature == "persistence":
            proof = directory / "persistence"
            example("persistence_proof", "seed", grpc, run_id, str(proof))
            for stage in ("resume", "finished"):
                game.send_signal(signal.SIGINT)
                game.wait(timeout=15)
                game = start("game-server", app_env, game_port)
                save(directory, f"doctor-{stage}.json", doctor(directory))
                example("persistence_proof", stage, grpc, run_id, str(proof))
        else:
            before = int(time.time() * 1_000_000) - 1_000_000
            for suffix in ("one", "two"):
                initial = http(directory, f"before-{suffix}", base, f"/users/{run_id}-{suffix}/ratings")
                require(all(value == {"rating": None, "completed_rated_games": 0}
                            for value in initial.values()), "fresh user has played games")
            example("rating_proof", grpc, run_id)
            log = next(directory.glob("rating_proof-*.log")).read_text()
            match = re.search(r"finished (\S+), black=(\S+), white=(\S+)", log)
            require(match is not None, "client did not identify the completed game")
            game_id, black, white = match.groups()
            if feature == "records":
                page = http(directory, "search", base,
                            f"/game-records?from={before}&to={int(time.time()*1_000_000)+1_000_000}"
                            f"&participant={black}&winner=black&reason=resignation&limit=1")
                require(len(page["items"]) == 1 and page["items"][0]["id"] == game_id,
                        "completed game missing from filtered search")
                detail = http(directory, "detail", base, f"/game-records/{game_id}")
                require(detail["metadata"] == page["items"][0] and
                        detail["history"] == [{"seq": 1, "player": "white", "action": "resign"}],
                        "detail disagrees with search or resignation history")
                http(directory, "missing", base, "/game-records/does-not-exist", 404)
                http(directory, "invalid-search", base, "/game-records?from=1&to=0", 400)
            else:
                start("rating-worker-server", app_env)
                last = {}
                def ratings_applied():
                    for side, user in (("black", black), ("white", white)):
                        last[side] = http(directory, f"after-{side}", base, f"/users/{user}/ratings")
                    return all(row["ten_minute_30"]["completed_rated_games"] == 1 for row in last.values())
                wait_for(ratings_applied, "one committed rated game per player")
                require(last["black"]["ten_minute_30"]["rating"] > last["white"]["ten_minute_30"]["rating"],
                        "winner rating must exceed loser rating from equal initial ratings")
                require(all(row["byoyomi_30"] == {"rating": None, "completed_rated_games": 0}
                            for row in last.values()), "unused pool changed")
        result["status"] = "passed"
    except BaseException as error:
        result["error"] = str(error)
        raise
    finally:
        cleanup_errors = []
        for child in reversed(processes):
            if child.poll() is None:
                child.send_signal(signal.SIGINT)
                try:
                    child.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
        for label, args in (("postgres-logs", ["logs", "--no-color", "postgres"]),
                            ("cleanup", ["down", "--volumes", "--remove-orphans"])):
            try:
                command(directory, label, compose + args, env)
            except Exception as error:
                cleanup_errors.append(str(error))
        if cleanup_errors:
            result["status"] = "failed"
        save(directory, "result.json", result | {"cleanup_errors": cleanup_errors})
        require(not cleanup_errors, "cleanup failed: " + "; ".join(cleanup_errors))
        require((directory / "result.json").is_file(), "evidence did not survive cleanup")
        print(json.dumps(result), flush=True)


def main():
    def interrupted(signum, frame):
        raise KeyboardInterrupt(f"received signal {signum}")
    signal.signal(signal.SIGTERM, interrupted)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["play", "persistence", "records", "ratings", "doctor"])
    parser.add_argument("directory", nargs="?", type=Path, help="live run directory for doctor")
    args = parser.parse_args()
    if args.action == "doctor":
        if args.directory is None:
            parser.error("doctor requires a live run directory")
        print(json.dumps(doctor(args.directory), indent=2))
    else:
        require(args.directory is None, "run directories are allocated automatically")
        run(args.action)


if __name__ == "__main__":
    main()
