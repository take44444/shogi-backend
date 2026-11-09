#!/usr/bin/env python3
import ipaddress
import json
import os
from pathlib import Path
import subprocess
from urllib.parse import urlsplit
import uuid


ROOT = Path(__file__).resolve().parents[1]


def main():
    environment = {**os.environ, "POSTGRES_PORT": "0"}
    compose = ["docker", "compose", "--profile", "*", "-p", f"shogi-permissions-{uuid.uuid4().hex[:12]}"]

    def run(*args, success=True):
        result = subprocess.run([*compose, *args], cwd=ROOT, env=environment, text=True, capture_output=True)
        if success and result.returncode:
            raise AssertionError(f"deployment command failed: {args[0]}\n{result.stderr}")
        return result

    configuration = json.loads(run("config", "--format", "json").stdout)
    services = configuration["services"]
    ports = services["game-server"]["ports"]
    assert ports and all(ipaddress.ip_address(port["host_ip"]).is_loopback for port in ports), "game gRPC must be published only on loopback"
    writer = urlsplit(services["game-server"]["environment"]["DATABASE_URL"])
    reader = urlsplit(services["player-server"]["environment"]["PLAYER_GAME_DATABASE_URL"])
    accounts = urlsplit(services["user-server"]["environment"]["USER_DATABASE_URL"])
    assert (writer.hostname, writer.port, writer.path) == (reader.hostname, reader.port, reader.path) and writer.path != accounts.path, "game and account databases must stay separate"
    assert writer.username != reader.username, "public reader must use separate credentials"

    def sql(user, database, statement, success=True):
        return run("exec", "-T", "postgres", "psql", "-U", user, "-d", database,
                   "-v", "ON_ERROR_STOP=1", "-v", "VERBOSITY=sqlstate",
                   "-c", "SET default_transaction_read_only=off", "-c", statement, success=success)

    try:
        run("up", "-d", "--wait", "postgres")
        sql(writer.username, writer.path[1:], "CREATE TABLE public.permission_probe (value INTEGER); INSERT INTO public.permission_probe VALUES (1)")
        sql(reader.username, reader.path[1:], "SELECT value FROM public.permission_probe")
        sql(accounts.username, accounts.path[1:], "SELECT 1")
        for database, statement, expected in [
            (reader.path[1:], "UPDATE public.permission_probe SET value=2", "42501"),
            (reader.path[1:], "CREATE TABLE public.forbidden_write (id INTEGER)", "42501"),
            (accounts.path[1:], "SELECT 1", "permission denied for database"),
        ]:
            result = sql(reader.username, database, statement, success=False)
            assert result.returncode != 0 and expected in result.stderr, f"reader operation must fail with insufficient_privilege: {database}: {result.stderr}"
        print("Deployment contracts passed: loopback gRPC, separate account DB, SELECT-only reader and denied account access.")
    finally:
        run("down", "--volumes", "--remove-orphans")


if __name__ == "__main__":
    main()
