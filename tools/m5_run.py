#!/usr/bin/env python3

from __future__ import annotations

import argparse
import codecs
import json
import os
import pty
import signal
import subprocess
import sys
import termios
import threading
import time
import tty
import urllib.error
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from harness.lib.llm_log import extract_llm_api_log, write_llm_api_log_jsonl
from harness.lib.protocol import TRACE_PREFIX


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
        try:
            proc.kill()
            proc.wait(timeout=timeout_sec)
        except subprocess.TimeoutExpired:
            print(f"warning: timed out killing {name}", file=sys.stderr)
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


def _extract_trace_events_from_text(text: str, trace_events: list[dict], trace_lines: list[str]) -> None:
    for raw_line in text.splitlines():
        idx = raw_line.find(TRACE_PREFIX)
        if idx == -1:
            continue
        raw_json = raw_line[idx + len(TRACE_PREFIX) :].strip()
        if not raw_json:
            continue
        trace_lines.append(raw_json)
        try:
            trace_events.append(json.loads(raw_json))
        except json.JSONDecodeError:
            continue


def _is_trace_line(text: str) -> bool:
    return text.lstrip("\r").startswith(TRACE_PREFIX)


def _could_be_trace_prefix(text: str) -> bool:
    stripped = text.lstrip("\r")
    if not stripped:
        return False
    return stripped.startswith(TRACE_PREFIX) or TRACE_PREFIX.startswith(stripped)


def cleanup_previous_run(state_path: Path) -> None:
    if not state_path.exists():
        return
    try:
        state = json.loads(state_path.read_text(encoding="utf-8"))
    except Exception:
        state_path.unlink(missing_ok=True)
        return
    terminate_process_group(state.get("runtime_pgid"), "previous MiniAgentOS runtime")
    terminate_process_group(state.get("bridge_pgid"), "previous host bridge")
    state_path.unlink(missing_ok=True)


def enable_cbreak_stdin() -> tuple[int, list] | None:
    if not sys.stdin.isatty():
        return None
    fd = sys.stdin.fileno()
    original = termios.tcgetattr(fd)
    tty.setcbreak(fd)
    return fd, original


def restore_stdin_mode(state: tuple[int, list] | None) -> None:
    if state is None:
        return
    fd, original = state
    try:
        termios.tcsetattr(fd, termios.TCSADRAIN, original)
    except termios.error:
        return


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Launch the MiniAgentOS live agent runtime")
    parser.add_argument(
        "--workspace",
        default=".",
        help="Workspace root exposed to bounded workspace tools. Defaults to the current directory.",
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
        help="Host bind address for the live host bridge. Default: 127.0.0.1.",
    )
    parser.add_argument(
        "--show-trace",
        action="store_true",
        help="Show raw guest TRACE lines in the terminal while still recording logs.",
    )
    parser.add_argument(
        "--no-trace-capture",
        action="store_true",
        help="Do not auto-enable guest trace capture. This also disables live llm_api_log updates.",
    )
    parser.add_argument(
        "--trace-on",
        action="store_true",
        help=argparse.SUPPRESS,
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

    logs_root = repo_root / "output" / "agent-manual" / time.strftime("%Y%m%d-%H%M%S")
    logs_root.mkdir(parents=True, exist_ok=True)
    bridge_log_path = logs_root / "m5_bridge.log"
    uart_log_path = logs_root / "uart.log"
    trace_log_path = logs_root / "trace.jsonl"
    llm_log_path = logs_root / "llm_api_log.jsonl"
    bridge_log = bridge_log_path.open("wb")
    state_path = repo_root / "output" / "agent-manual" / "current.json"
    legacy_state_path = repo_root / "output" / "m5-manual" / "current.json"

    cleanup_previous_run(state_path)
    cleanup_previous_run(legacy_state_path)

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
    runtime_master_fd: int | None = None
    runtime_reader: threading.Thread | None = None
    runtime_stdin_writer: threading.Thread | None = None
    runtime_text_chunks: list[str] = []
    runtime_trace_lines: list[str] = []
    runtime_trace_events: list[dict] = []
    runtime_reader_error: list[BaseException] = []
    runtime_auto_trace_sent = False
    runtime_output_lock = threading.Lock()
    stdin_mode_state = enable_cbreak_stdin()
    trace_capture_enabled = not args.no_trace_capture
    if args.trace_on:
        args.show_trace = True

    def refresh_trace_artifacts() -> None:
        trace_log_path.write_text(
            "".join(line + "\n" for line in runtime_trace_lines),
            encoding="utf-8",
        )
        llm_rows = extract_llm_api_log(runtime_trace_events)
        if llm_rows:
            write_llm_api_log_jsonl(llm_log_path, llm_rows)
        elif llm_log_path.exists():
            llm_log_path.unlink(missing_ok=True)

    def cleanup() -> None:
        nonlocal runtime_master_fd
        terminate_process(runtime_proc, "MiniAgentOS runtime")
        terminate_process(bridge_proc, "host bridge")
        terminate_process_group(
            os.getpgid(runtime_proc.pid) if runtime_proc is not None and runtime_proc.poll() is None else None,
            "MiniAgentOS runtime",
        )
        terminate_process_group(
            os.getpgid(bridge_proc.pid) if bridge_proc is not None and bridge_proc.poll() is None else None,
            "host bridge",
        )
        state_path.unlink(missing_ok=True)
        if runtime_master_fd is not None:
            try:
                os.close(runtime_master_fd)
            except OSError:
                pass
            runtime_master_fd = None
        try:
            bridge_log.close()
        except Exception:
            pass

    def write_runtime_logs() -> None:
        uart_log_path.write_text("".join(runtime_text_chunks), encoding="utf-8")
        refresh_trace_artifacts()

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
        print(f"Workspace: {workspace_root}")
        print(f"Host bridge: http://{args.bind}:{args.bridge_port}")
        print(f"Host bridge log: {bridge_log_path}")
        print(f"LLM API log: {llm_log_path}")
        print("Recommended first smoke inside Goal > :")
        print("  m5-status")
        print("  m5-list")
        print("  m5-read app.py")
        print("  m6-search Brave Search API authentication header")
        print("Recommended coding task:")
        print("  Run check.py first. If it fails, fix the minimum code needed in the workspace and run check.py again until it passes.")
        print("")
        master_fd, slave_fd = pty.openpty()
        runtime_master_fd = master_fd
        runtime_proc = subprocess.Popen(
            ["make", "run-net-legacy"],
            cwd=repo_root / "runtime",
            env=env,
            stdin=slave_fd,
            stdout=slave_fd,
            stderr=slave_fd,
            start_new_session=True,
        )
        os.close(slave_fd)

        def runtime_reader_loop() -> None:
            nonlocal runtime_auto_trace_sent
            decoder = codecs.getincrementaldecoder("utf-8")(errors="replace")
            buffer = ""
            display_buffer = ""
            bootstrap_hide_until_prompt = False
            try:
                def flush_display_text(text: str) -> None:
                    nonlocal display_buffer, bootstrap_hide_until_prompt, runtime_auto_trace_sent
                    display_buffer += text
                    while True:
                        if trace_capture_enabled and not runtime_auto_trace_sent:
                            prompt_idx = display_buffer.find("Goal > ")
                            if prompt_idx == -1:
                                safe_len = max(0, len(display_buffer) - len("Goal > "))
                                if safe_len > 0:
                                    sys.stdout.write(display_buffer[:safe_len])
                                    sys.stdout.flush()
                                    display_buffer = display_buffer[safe_len:]
                                return
                            if prompt_idx > 0:
                                sys.stdout.write(display_buffer[:prompt_idx])
                                sys.stdout.flush()
                            display_buffer = display_buffer[prompt_idx + len("Goal > ") :]
                            try:
                                os.write(master_fd, b"trace on\n")
                                runtime_auto_trace_sent = True
                                bootstrap_hide_until_prompt = True
                            except OSError:
                                pass
                            continue

                        if bootstrap_hide_until_prompt:
                            prompt_idx = display_buffer.rfind("Goal > ")
                            if prompt_idx == -1:
                                return
                            prompt_end = prompt_idx + len("Goal > ")
                            if prompt_end != len(display_buffer):
                                return
                            display_buffer = ""
                            sys.stdout.write("Goal > ")
                            sys.stdout.flush()
                            bootstrap_hide_until_prompt = False
                            continue

                        while "\n" in display_buffer:
                            line, display_buffer = display_buffer.split("\n", 1)
                            full_line = line + "\n"
                            if args.show_trace or not _is_trace_line(full_line):
                                sys.stdout.write(full_line)
                                sys.stdout.flush()

                        if display_buffer and not _could_be_trace_prefix(display_buffer):
                            sys.stdout.write(display_buffer)
                            sys.stdout.flush()
                            display_buffer = ""
                        return

                while True:
                    chunk = os.read(master_fd, 4096)
                    if not chunk:
                        break
                    text = decoder.decode(chunk)
                    if not text:
                        continue
                    with runtime_output_lock:
                        runtime_text_chunks.append(text)
                    buffer += text
                    flush_display_text(text)
                    while "\n" in buffer:
                        line, buffer = buffer.split("\n", 1)
                        full_line = line + "\n"
                        trace_count_before = len(runtime_trace_events)
                        _extract_trace_events_from_text(
                            full_line,
                            runtime_trace_events,
                            runtime_trace_lines,
                        )
                        if len(runtime_trace_events) != trace_count_before:
                            refresh_trace_artifacts()
                tail = decoder.decode(b"", final=True)
                if tail:
                    with runtime_output_lock:
                        runtime_text_chunks.append(tail)
                    buffer += tail
                    flush_display_text(tail)
                if buffer:
                    trace_count_before = len(runtime_trace_events)
                    _extract_trace_events_from_text(
                        buffer,
                        runtime_trace_events,
                        runtime_trace_lines,
                    )
                    if len(runtime_trace_events) != trace_count_before:
                        refresh_trace_artifacts()
            except BaseException as exc:  # pragma: no cover - defensive manual path
                runtime_reader_error.append(exc)

        def runtime_stdin_loop() -> None:
            try:
                while True:
                    data = os.read(sys.stdin.fileno(), 1024)
                    if not data:
                        break
                    os.write(master_fd, data)
            except OSError:
                return

        runtime_reader = threading.Thread(target=runtime_reader_loop, daemon=True)
        runtime_reader.start()
        runtime_stdin_writer = threading.Thread(target=runtime_stdin_loop, daemon=True)
        runtime_stdin_writer.start()
        runtime_pgid = os.getpgid(runtime_proc.pid)
        state_path.write_text(
            json.dumps(
                {
                    "started_at": int(time.time()),
                    "workspace": str(workspace_root),
                    "bridge_log": str(bridge_log_path),
                    "uart_log": str(uart_log_path),
                    "trace_log": str(trace_log_path),
                    "llm_api_log": str(llm_log_path),
                    "bridge_pgid": bridge_pgid,
                    "runtime_pgid": runtime_pgid,
                },
                indent=2,
                ensure_ascii=False,
            )
            + "\n",
            encoding="utf-8",
        )
        exit_code = runtime_proc.wait()
        if runtime_reader is not None:
            runtime_reader.join(timeout=2.0)
        if runtime_reader_error:
            raise runtime_reader_error[0]
        return exit_code
    finally:
        write_runtime_logs()
        cleanup()
        restore_stdin_mode(stdin_mode_state)
        signal.signal(signal.SIGINT, old_sigint)
        signal.signal(signal.SIGTERM, old_sigterm)


if __name__ == "__main__":
    raise SystemExit(main())
