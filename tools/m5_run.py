#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import os
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path


def shell_env_value(name: str) -> str | None:
    value = os.environ.get(name)
    if value:
        return value
    for shell in ("zsh", "bash"):
        try:
            proc = subprocess.run(
                [shell, "-ic", f'printf %s "${{{name}:-}}"'],
                capture_output=True,
                text=True,
                timeout=5,
                check=False,
            )
        except (FileNotFoundError, subprocess.TimeoutExpired):
            continue
        if proc.returncode == 0 and proc.stdout:
            return proc.stdout
    return None


def load_runtime_m5_config(repo_root: Path) -> dict:
    config_path = repo_root / "harness" / "config.runtime-m5.json"
    return json.loads(config_path.read_text(encoding="utf-8"))


def build_launch_env(config_data: dict) -> dict[str, str]:
    env = os.environ.copy()
    prefixes: list[str] = []

    for raw in config_data.get("path_prefixes", []):
        expanded = os.path.expandvars(raw)
        if expanded and Path(expanded).exists() and expanded not in prefixes:
            prefixes.append(expanded)

    for candidate in (
        Path("/opt/homebrew/bin"),
        Path.home() / "homebrew" / "bin",
        Path.home() / ".cargo" / "bin",
    ):
        value = str(candidate)
        if candidate.exists() and value not in prefixes:
            prefixes.append(value)

    current_path = env.get("PATH", "")
    if prefixes:
        env["PATH"] = os.pathsep.join(prefixes + ([current_path] if current_path else []))

    for key, value in config_data.get("env", {}).items():
        env[str(key)] = str(value)

    return env


def wait_for_http_ok(url: str, timeout_sec: float) -> None:
    deadline = time.time() + timeout_sec
    last_error = None
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=1.0) as response:
                if 200 <= response.status < 300:
                    return
        except (urllib.error.URLError, TimeoutError, OSError) as exc:
            last_error = exc
        time.sleep(0.1)
    raise RuntimeError(f"timed out waiting for {url}: {last_error}")


def terminate_process(proc: subprocess.Popen | None, name: str, timeout_sec: float = 5.0) -> None:
    if proc is None or proc.poll() is not None:
        return
    try:
        proc.terminate()
        proc.wait(timeout=timeout_sec)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=timeout_sec)
    except ProcessLookupError:
        return
    except Exception as exc:
        print(f"warning: failed to stop {name}: {exc}", file=sys.stderr)


def terminate_process_group(pgid: int | None, name: str, timeout_sec: float = 5.0) -> None:
    if not pgid:
        return
    try:
        os.killpg(pgid, signal.SIGTERM)
    except ProcessLookupError:
        return
    deadline = time.time() + timeout_sec
    while time.time() < deadline:
        try:
            os.killpg(pgid, 0)
        except ProcessLookupError:
            return
        time.sleep(0.1)
    try:
        os.killpg(pgid, signal.SIGKILL)
    except ProcessLookupError:
        return


def cleanup_previous_run(state_path: Path) -> None:
    if not state_path.exists():
        return
    try:
        state = json.loads(state_path.read_text(encoding="utf-8"))
    except Exception:
        state_path.unlink(missing_ok=True)
        return
    terminate_process_group(state.get("runtime_pgid"), "previous MiniAgentOS runtime")
    terminate_process_group(state.get("bridge_pgid"), "previous M5 bridge")
    state_path.unlink(missing_ok=True)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Launch MiniAgentOS M5 with host bridge and live OpenAI settings")
    parser.add_argument(
        "--workspace",
        default=".",
        help="Workspace root exposed to M5 tools. Defaults to the current directory.",
    )
    parser.add_argument(
        "--proxy-port",
        default="10808",
        help="Host SOCKS5 proxy port for the native guest OpenAI path. Default: 10808.",
    )
    parser.add_argument(
        "--bridge-port",
        type=int,
        default=8090,
        help="Host bridge port exposed to the guest. Default: 8090.",
    )
    parser.add_argument(
        "--python-image",
        default="python:3.12-slim",
        help="Docker image used for bounded Python runs. Default: python:3.12-slim.",
    )
    parser.add_argument(
        "--bind",
        default="127.0.0.1",
        help="Host bind address for the M5 bridge. Default: 127.0.0.1.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parents[1]
    workspace_root = Path(args.workspace).resolve()
    if not workspace_root.is_dir():
        print(f"workspace is not a directory: {workspace_root}", file=sys.stderr)
        return 2

    api_key = shell_env_value("OPENAI_API_KEY")
    if not api_key:
        print("OPENAI_API_KEY was not found in the current environment or an interactive zsh/bash shell", file=sys.stderr)
        return 2

    config_data = load_runtime_m5_config(repo_root)
    env = build_launch_env(config_data)
    env["OPENAI_API_KEY"] = api_key
    env["MINIOS_USE_HOST_SOCKS5_PROXY"] = "1"
    env["MINIOS_HOST_SOCKS5_PORT"] = str(args.proxy_port)
    env["MINIOS_USE_OPENAI_HOST_BRIDGE"] = "0"
    env["MINIOS_ENABLE_NATIVE_OPENAI_TRANSPORT_REUSE"] = "0"
    env["MINIOS_DISABLE_AUTO_TLS_LOCAL_FETCH"] = "1"

    logs_root = repo_root / "output" / "m5-manual" / time.strftime("%Y%m%d-%H%M%S")
    logs_root.mkdir(parents=True, exist_ok=True)
    bridge_log_path = logs_root / "m5_bridge.log"
    bridge_log = bridge_log_path.open("wb")
    state_path = repo_root / "output" / "m5-manual" / "current.json"

    cleanup_previous_run(state_path)

    bridge_command = [
        env.get("PYTHON", "python3"),
        str(repo_root / "tools" / "m5_host_bridge.py"),
        "--workspace",
        str(workspace_root),
        "--bind",
        args.bind,
        "--port",
        str(args.bridge_port),
        "--python-image",
        args.python_image,
        "--output-dir",
        str(logs_root),
    ]

    bridge_proc: subprocess.Popen | None = None
    runtime_proc: subprocess.Popen | None = None

    def cleanup() -> None:
        terminate_process(runtime_proc, "MiniAgentOS runtime")
        terminate_process(bridge_proc, "M5 bridge")
        terminate_process_group(
            os.getpgid(runtime_proc.pid) if runtime_proc is not None and runtime_proc.poll() is None else None,
            "MiniAgentOS runtime",
        )
        terminate_process_group(
            os.getpgid(bridge_proc.pid) if bridge_proc is not None and bridge_proc.poll() is None else None,
            "M5 bridge",
        )
        state_path.unlink(missing_ok=True)
        try:
            bridge_log.close()
        except Exception:
            pass

    def handle_signal(signum, _frame) -> None:
        cleanup()
        raise SystemExit(128 + signum)

    old_sigint = signal.signal(signal.SIGINT, handle_signal)
    old_sigterm = signal.signal(signal.SIGTERM, handle_signal)

    try:
        bridge_proc = subprocess.Popen(
            bridge_command,
            cwd=repo_root,
            env=env,
            stdout=bridge_log,
            stderr=subprocess.STDOUT,
            text=False,
            bufsize=0,
            start_new_session=True,
        )
        wait_for_http_ok(f"http://{args.bind}:{args.bridge_port}/healthz", 10.0)

        bridge_pgid = os.getpgid(bridge_proc.pid)
        print(f"M5 workspace: {workspace_root}")
        print(f"M5 bridge: http://{args.bind}:{args.bridge_port}")
        print(f"M5 bridge log: {bridge_log_path}")
        print("Recommended first smoke inside Goal > :")
        print("  m5-status")
        print("  m5-list")
        print("  m5-read app.py")
        print("Recommended coding task:")
        print("  Run check.py first. If it fails, fix the minimum code needed in the workspace and run check.py again until it passes.")
        print("")

        runtime_proc = subprocess.Popen(
            ["make", "run-net-legacy"],
            cwd=repo_root / "runtime",
            env=env,
            start_new_session=True,
        )
        runtime_pgid = os.getpgid(runtime_proc.pid)
        state_path.write_text(
            json.dumps(
                {
                    "started_at": int(time.time()),
                    "workspace": str(workspace_root),
                    "bridge_log": str(bridge_log_path),
                    "bridge_pgid": bridge_pgid,
                    "runtime_pgid": runtime_pgid,
                },
                indent=2,
                ensure_ascii=False,
            )
            + "\n",
            encoding="utf-8",
        )
        return runtime_proc.wait()
    finally:
        cleanup()
        signal.signal(signal.SIGINT, old_sigint)
        signal.signal(signal.SIGTERM, old_sigterm)


if __name__ == "__main__":
    raise SystemExit(main())
