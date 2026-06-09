#!/usr/bin/env python3
"""Run CLI black-box checks against a fresh local server instance.

This suite is intentionally stdlib-only so developers can run it locally without
installing pytest. It starts a disposable PostgreSQL container, starts the Go
server with test values, builds the Rust CLI, then exercises the CLI with
isolated temp profiles.
"""

from __future__ import annotations

import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SERVER_URL = os.environ.get("TASKMANAGER_E2E_SERVER_URL", "http://127.0.0.1:18080")
DATABASE_URL = os.environ.get(
    "TASKMANAGER_E2E_DATABASE_URL",
    "postgres://tasks:tasks@localhost:5432/tasks?sslmode=disable",
)
JWT_SECRET = os.environ.get(
    "TASKMANAGER_E2E_JWT_SECRET",
    "taskmanager-cli-e2e-test-secret-change-me-32-bytes",
)


def run(cmd: list[str], **kwargs) -> subprocess.CompletedProcess[str]:
    print("$", " ".join(cmd))
    return subprocess.run(
        cmd,
        cwd=kwargs.pop("cwd", ROOT),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        **kwargs,
    )


def require(cmd: list[str], **kwargs) -> subprocess.CompletedProcess[str]:
    result = run(cmd, **kwargs)
    if result.returncode != 0:
        print(result.stdout)
        print(result.stderr, file=sys.stderr)
        raise SystemExit(result.returncode)
    return result


def wait_for_http(url: str, timeout: float = 30.0) -> None:
    deadline = time.time() + timeout
    last_error: Exception | None = None
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=1) as response:
                if response.status == 200:
                    return
        except Exception as error:  # noqa: BLE001 - report last startup error
            last_error = error
        time.sleep(0.5)
    raise RuntimeError(f"timed out waiting for {url}: {last_error}")


def wait_for_postgres(timeout: float = 30.0) -> None:
    deadline = time.time() + timeout
    last_stderr = ""
    while time.time() < deadline:
        result = run(
            ["docker", "compose", "exec", "-T", "postgres", "pg_isready", "-U", "tasks", "-d", "tasks"],
            cwd=ROOT / "server",
        )
        if result.returncode == 0:
            return
        last_stderr = result.stderr.strip() or result.stdout.strip()
        time.sleep(0.5)
    raise RuntimeError(f"timed out waiting for postgres: {last_stderr}")


def read_process_output(process: subprocess.Popen[str]) -> str:
    if process.stdout is None:
        return ""
    try:
        return process.stdout.read() or ""
    except Exception as error:  # noqa: BLE001 - best-effort diagnostics
        return f"<failed to read process output: {error}>"


def cli(base_env: dict[str, str], profile_dir: Path, *args: str, expect: int = 0) -> subprocess.CompletedProcess[str]:
    env = base_env.copy()
    env["TASKMANAGER_INSECURE_KEY_DIR"] = str(profile_dir / "keys")
    env["TASKMANAGER_REMINDER_DIR"] = str(profile_dir / "reminders")
    cmd = [
        str(ROOT / "target" / "debug" / "taskmanager"),
        "--server",
        SERVER_URL,
        "--db",
        str(profile_dir / "tasks.db"),
        "--output",
        "json",
        *args,
    ]
    result = run(cmd, env=env)
    if result.returncode != expect:
        print(result.stdout)
        print(result.stderr, file=sys.stderr)
        raise AssertionError(f"expected exit {expect}, got {result.returncode}: {' '.join(args)}")
    return result


def parse_result(result: subprocess.CompletedProcess[str]) -> dict:
    return json.loads(result.stdout)["result"]


def assert_error(result: subprocess.CompletedProcess[str], code: str) -> None:
    payload = json.loads(result.stderr)
    assert payload["error"]["code"] == code, payload


def main() -> int:
    if shutil.which("docker") is None:
        print("docker is required for the CLI E2E suite", file=sys.stderr)
        return 1
    if shutil.which("go") is None:
        print("go is required for the CLI E2E suite", file=sys.stderr)
        return 1
    if shutil.which("cargo") is None:
        print("cargo is required for the CLI E2E suite", file=sys.stderr)
        return 1

    require(["cargo", "build", "-p", "taskmanager-cli"])

    server_proc: subprocess.Popen[str] | None = None
    with tempfile.TemporaryDirectory(prefix="taskmanager-cli-e2e-") as tmp:
        tmpdir = Path(tmp)
        profile_a = tmpdir / "profile-a"
        profile_b = tmpdir / "profile-b"
        profile_a.mkdir()
        profile_b.mkdir()

        env = os.environ.copy()
        env.update(
            {
                "PORT": "18080",
                "DATABASE_URL": DATABASE_URL,
                "JWT_SECRET": JWT_SECRET,
                "JWT_ISSUER": "tasks-cli-e2e",
                "ACCESS_TOKEN_TTL": "15m",
                "REFRESH_TOKEN_TTL": "720h",
                "WRITE_RATE_LIMIT_PER_MIN": "1000",
                "MAX_BLOB_BYTES": "1048576",
                "MAX_BATCH_BLOBS": "100",
                "TOMBSTONE_RETENTION": "720h",
            }
        )

        try:
            require(["docker", "compose", "down", "-v"], cwd=ROOT / "server")
            require(["docker", "compose", "up", "-d", "postgres"], cwd=ROOT / "server")
            wait_for_postgres()
            server_proc = subprocess.Popen(
                ["go", "run", "./cmd/server"],
                cwd=ROOT / "server",
                env=env,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
            )
            try:
                wait_for_http(f"{SERVER_URL}/healthz")
            except Exception:
                if server_proc.poll() is not None:
                    print("\nserver exited before becoming healthy; output:", file=sys.stderr)
                    print(read_process_output(server_proc), file=sys.stderr)
                raise

            version = parse_result(cli(env, profile_a, "version"))
            assert version["name"] == "taskmanager-cli"

            account_a = parse_result(cli(env, profile_a, "account", "init"))
            assert account_a["public_key"]
            rerun = cli(env, profile_a, "account", "init", expect=5)
            assert_error(rerun, "conflict")

            login = parse_result(
                cli(
                    env,
                    profile_a,
                    "auth",
                    "login",
                    "--access-token",
                    "test-access",
                    "--refresh-token",
                    "test-refresh",
                )
            )
            assert login["stored"] is True

            created = parse_result(
                cli(
                    env,
                    profile_a,
                    "task",
                    "create",
                    "--title",
                    "server e2e task",
                    "--body",
                    "created while server is running",
                    "--tag",
                    "e2e",
                )
            )
            task_id = created["id"]
            assert created["dirty"] is True

            listed = parse_result(cli(env, profile_a, "task", "list"))
            assert any(task["id"] == task_id for task in listed)

            status = parse_result(cli(env, profile_a, "sync", "status"))
            assert status["dirty_count"] == 1
            retry = parse_result(cli(env, profile_a, "sync", "retry", task_id))
            assert retry["attempt"] == 1
            assert retry["task_id"] == task_id

            device_b = parse_result(cli(env, profile_b, "device", "init-keypair"))
            wrapped = parse_result(cli(env, profile_a, "device", "wrap-key", "--target", device_b["public_key"]))
            unwrapped = parse_result(
                cli(
                    env,
                    profile_b,
                    "device",
                    "unwrap-key",
                    "--from",
                    account_a["public_key"],
                    "--ciphertext",
                    wrapped["ciphertext"],
                    "--nonce",
                    wrapped["nonce"],
                )
            )
            assert unwrapped["stored"] is True

            malformed = cli(env, profile_a, "device", "wrap-key", "--target", "éé", expect=1)
            assert_error(malformed, "input_error")

            sync_push = cli(env, profile_a, "sync", "push", expect=6)
            assert_error(sync_push, "unsupported_platform")

            logout = parse_result(cli(env, profile_a, "auth", "logout"))
            assert logout["logged_out"] is True

            print("CLI E2E suite passed")
            return 0
        finally:
            if server_proc is not None and server_proc.poll() is None:
                server_proc.send_signal(signal.SIGTERM)
                try:
                    server_proc.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    server_proc.kill()
            run(["docker", "compose", "down", "-v"], cwd=ROOT / "server")


if __name__ == "__main__":
    raise SystemExit(main())
