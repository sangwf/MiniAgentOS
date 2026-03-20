from __future__ import annotations

import argparse
import codecs
import json
import os
import queue
import re
import shutil
import subprocess
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path

from harness.lib.evaluator import evaluate_case
from harness.lib.http_fixtures import (
    decode_result_payload,
    start_interpretation_gateway,
    start_model_gateway,
    start_result_sink,
    start_source_fixture,
    start_x_fixture,
)
from harness.lib.llm_log import extract_llm_api_log, write_llm_api_log_jsonl
from harness.lib.protocol import (
    TRACE_PREFIX,
    build_input_line,
    build_turns,
    substitute_placeholders,
)


def _load_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def _shell_env_value(name: str) -> str | None:
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
            )
        except (FileNotFoundError, subprocess.TimeoutExpired):
            continue
        if proc.returncode == 0 and proc.stdout:
            return proc.stdout
    return None


def _reader(stdout, out_queue, log_chunks, raw_chunks):
    decoder = codecs.getincrementaldecoder("utf-8")(errors="replace")
    try:
        while True:
            chunk = os.read(stdout.fileno(), 4096)
            if not chunk:
                break
            raw_chunks.append(chunk)
            text = decoder.decode(chunk)
            if text:
                log_chunks.append(text)
                out_queue.put(text)
        tail = decoder.decode(b"", final=True)
        if tail:
            log_chunks.append(tail)
            out_queue.put(tail)
    finally:
        out_queue.put(None)


def _write_json(path: Path, payload):
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def _snapshot_workspace(root: Path):
    entries: dict[str, dict] = {}
    if not root.exists():
        return {"entries": entries}
    for path in sorted(root.rglob("*")):
        rel = path.relative_to(root).as_posix()
        if path.is_dir():
            entries[rel] = {"kind": "dir"}
            continue
        entry = {"kind": "file", "size": path.stat().st_size}
        try:
            content = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            content = None
        if content is not None and len(content.encode("utf-8")) <= 32768:
            entry["content"] = content
        entries[rel] = entry
    return {"entries": entries}


def _redact_uart_artifacts(uart_text: str, uart_raw: bytes, secrets: list[str]):
    redacted_text = uart_text
    redacted_raw = uart_raw
    for secret in secrets:
        if not secret:
            continue
        redacted_text = redacted_text.replace(secret, "[REDACTED]")
        redacted_raw = redacted_raw.replace(secret.encode("utf-8"), b"[REDACTED]")
    return redacted_text, redacted_raw


def _has_terminal_trace(trace_events):
    for event in trace_events:
        if not isinstance(event, dict):
            continue
        if event.get("event") in {"goal_completed", "goal_refused", "goal_failed"}:
            return True
    return False


def _launch_env(config_data: dict):
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


def _wait_for_http_ok(url: str, timeout_sec: float) -> None:
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


def _terminal_status(trace_events):
    for event in reversed(trace_events):
        if not isinstance(event, dict):
            continue
        name = event.get("event")
        if name == "goal_completed":
            return "ok"
        if name == "goal_refused":
            return "refused"
        if name == "goal_failed":
            return "error"
        if name == "loop_stopped":
            stop_reason = event.get("stop_reason")
            if stop_reason in {"final_response"}:
                return "ok"
            if stop_reason in {"unsupported", "policy_denied"}:
                return "refused"
            if stop_reason:
                return "error"
    return None


def _extract_terminal_result(uart_text: str, trace_events: list[dict]) -> dict | None:
    normalized_text = uart_text.replace("\r\n", "\n").replace("\r", "\n")
    result: dict[str, str] = {}
    summary_matches = list(
        re.finditer(
            r"(?:^|\n)--- summary ---\n(?P<summary>.*?)(?=\nTRACE |\nGoal >|\nreason:|\Z)",
            normalized_text,
            flags=re.S,
        )
    )
    if summary_matches:
        summary = summary_matches[-1].group("summary").strip()
        if summary:
            result["summary"] = summary

    reason_matches = list(re.finditer(r"(?:^|\n)reason: (?P<reason>.+)", normalized_text))
    if reason_matches:
        reason = reason_matches[-1].group("reason").strip()
        if reason:
            result["reason"] = reason

    if "summary" not in result:
        lines = normalized_text.splitlines()
        completion_index = None
        for index in range(len(lines) - 1, -1, -1):
            if re.search(
                r'^TRACE \{"event":"(?:goal_completed|goal_refused|goal_failed|loop_stopped)"',
                lines[index],
            ):
                completion_index = index
                break
        if completion_index is not None:
            summary_lines: list[str] = []
            for index in range(completion_index - 1, -1, -1):
                line = lines[index].rstrip()
                if not line:
                    if summary_lines:
                        break
                    continue
                if line.startswith("TRACE ") or line.startswith("Goal >"):
                    if summary_lines:
                        break
                    continue
                if line.startswith("reason: "):
                    break
                summary_lines.append(line)
            if summary_lines:
                result["summary"] = "\n".join(reversed(summary_lines)).strip()

    status = _terminal_status(trace_events)
    if status:
        result["status"] = status

    return result or None


def _extract_intent_ir(trace_events: list[dict]) -> dict | None:
    for event in reversed(trace_events):
        if not isinstance(event, dict):
            continue
        if event.get("event") != "intent_compiled":
            continue
        return {
            key: value
            for key, value in event.items()
            if key not in {"event", "ts_ms", "step"}
        }
    return None


def _extract_tool_calls(trace_events: list[dict]) -> list[dict]:
    tool_events = {
        "tool_call_requested",
        "tool_call_started",
        "tool_call_completed",
        "tool_call_denied",
    }
    extracted: list[dict] = []
    for event in trace_events:
        if not isinstance(event, dict):
            continue
        if event.get("event") not in tool_events:
            continue
        extracted.append(
            {
                key: value
                for key, value in event.items()
                if key not in {"ts_ms"}
            }
        )
    return extracted


def _extract_memory_snapshot(trace_events: list[dict]) -> dict | None:
    entries_by_id: dict[str, dict] = {}
    ordered_ids: list[str] = []
    for event in trace_events:
        if not isinstance(event, dict) or event.get("event") != "memory_entry_snapshot":
            continue
        entry_id = str(event.get("id", "")).strip()
        if not entry_id:
            continue
        entry = {
            "id": entry_id,
            "kind": event.get("kind"),
            "summary": event.get("summary"),
            "source": event.get("source"),
            "state": event.get("state"),
        }
        for field in ("created_turn", "updated_turn", "chars", "estimated_tokens"):
            if field in event:
                entry[field] = event.get(field)
        entries_by_id[entry_id] = entry
        if entry_id not in ordered_ids:
            ordered_ids.append(entry_id)
    if not entries_by_id:
        return None
    return {"entries": [entries_by_id[entry_id] for entry_id in ordered_ids]}


def _extract_memory_events(trace_events: list[dict]) -> dict | None:
    events: list[dict] = []
    for event in trace_events:
        if not isinstance(event, dict):
            continue
        if event.get("event") == "memory_event":
            item = {
                "turn_index": event.get("turn_index"),
                "event": "memory_updated",
                "entry_id": event.get("entry_id"),
            }
            for field in ("kind", "from_state", "to_state"):
                if field in event:
                    item[field] = event.get(field)
            events.append(item)
        elif event.get("event") == "memory_compacted":
            item = {
                "turn_index": event.get("turn_index"),
                "event": "memory_compacted",
                "entry_id": event.get("entry_id"),
            }
            for field in (
                "kind",
                "from_state",
                "to_state",
                "mode",
                "source_chars",
                "retained_chars",
                "dropped_chars",
            ):
                if field in event:
                    item[field] = event.get(field)
            events.append(item)
    if not events:
        return None
    return {"events": events}


def _extract_context_snapshot(trace_events: list[dict]) -> dict | None:
    order: list[int] = []
    by_interaction: dict[int, dict] = {}
    for event in trace_events:
        if not isinstance(event, dict) or event.get("event") != "context_section_snapshot":
            continue
        interaction_id = int(event.get("interaction_id") or 0)
        if interaction_id == 0:
            interaction_id = len(order) + 1
        if interaction_id not in by_interaction:
            by_interaction[interaction_id] = {"turn_index": interaction_id, "sections": []}
            order.append(interaction_id)
        by_interaction[interaction_id]["sections"].append(
            {
                "name": event.get("name"),
                "chars": event.get("chars"),
            }
        )
    if not by_interaction:
        return None
    return {"turns": [by_interaction[interaction_id] for interaction_id in order]}


def _extract_context_budget(trace_events: list[dict]) -> dict | None:
    turns: list[dict] = []
    turn_index = 0
    for event in trace_events:
        if not isinstance(event, dict) or event.get("event") != "context_budget_snapshot":
            continue
        turn_index += 1
        item = {"turn_index": turn_index}
        for field in (
            "instructions_chars",
            "current_request_chars",
            "latest_tool_result_chars",
            "working_memory_chars",
            "known_sources_chars",
            "workspace_memory_chars",
            "session_state_chars",
            "recent_conversation_chars",
            "estimated_total_tokens",
        ):
            if field in event:
                item[field] = event.get(field)
        turns.append(item)
    if not turns:
        return None
    return {"turns": turns}


def _synthesize_fetched_sources(session_transcript: list[dict], search_results) -> dict | None:
    url_to_result_id: dict[str, str] = {}
    if isinstance(search_results, dict):
        for search in search_results.get("searches", []):
            if not isinstance(search, dict):
                continue
            for result in search.get("results", []):
                if not isinstance(result, dict):
                    continue
                url = str(result.get("url", "")).strip()
                result_id = str(result.get("id", "")).strip()
                if url and result_id and url not in url_to_result_id:
                    url_to_result_id[url] = result_id
    synthesized: list[dict] = []
    for turn in session_transcript:
        turn_index = int(turn.get("turn", 0))
        tool_calls = turn.get("tool_calls", [])
        if not isinstance(tool_calls, list):
            continue
        for tool_call_index, tool_call in enumerate(tool_calls):
            if not isinstance(tool_call, dict):
                continue
            if tool_call.get("event") != "tool_call_requested":
                continue
            if tool_call.get("tool") != "fetch_url":
                continue
            arguments = tool_call.get("arguments", {})
            if not isinstance(arguments, dict):
                continue
            url = str(arguments.get("url", "")).strip()
            if not url:
                continue
            synthesized.append(
                {
                    "turn_index": turn_index,
                    "tool_call_index": tool_call_index,
                    "search_result_id": url_to_result_id.get(url),
                    "url": url,
                    "status_code": None,
                    "content_type": None,
                    "content_preview": "",
                    "truncated": True,
                }
            )
    return {"sources": synthesized} if synthesized else None


def _detect_guest_exception(uart_text: str) -> str | None:
    normalized_text = uart_text.replace("\r\n", "\n").replace("\r", "\n")
    match = re.search(r"(?:^|\n)(exception vector: .+)", normalized_text)
    if not match:
        return None
    return match.group(1).strip()


def run_case(case_path: Path, config_path: Path, output_dir: Path):
    root = Path(__file__).resolve().parents[2]
    case_path = case_path.resolve()
    config_path = config_path.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    case_data = _load_json(case_path)
    config_data = _load_json(config_path)
    case_dir = case_path.parent
    guest_openai_env = config_data.get("guest_openai_key_env")
    if case_data.get("requires_guest_openai"):
        if not guest_openai_env:
            raise ValueError("case requires guest_openai_key_env in harness config")
        env_name = str(guest_openai_env)
        if not _shell_env_value(env_name):
            raise ValueError(f"case requires {env_name} to be set")

    source_asset = case_data.get("fixture", {}).get("source_asset")
    if not source_asset:
        raise ValueError("case is missing fixture.source_asset")
    source_path = case_dir / source_asset
    source_content = source_path.read_text(encoding="utf-8")
    source_pages = None
    source_pages_asset = case_data.get("fixture", {}).get("source_pages_asset")
    if source_pages_asset:
        source_pages_path = case_dir / source_pages_asset
        source_pages = _load_json(source_pages_path)
        if not isinstance(source_pages, dict):
            raise ValueError("case fixture.source_pages_asset must be a JSON object")
        source_pages = {str(key): str(value) for key, value in source_pages.items()}

    search_fixture_path = None
    search_asset = case_data.get("fixture", {}).get("search_asset")
    if search_asset:
        search_fixture_path = (case_dir / search_asset).resolve()
        if not search_fixture_path.exists():
            raise ValueError("case fixture.search_asset was not found")

    workspace_root = None
    workspace_before = None
    workspace_asset = case_data.get("fixture", {}).get("workspace_asset")
    if workspace_asset:
        workspace_source = case_dir / workspace_asset
        if not workspace_source.exists() or not workspace_source.is_dir():
            raise ValueError("case fixture.workspace_asset must point to a directory")
        workspace_root = output_dir / "workspace"
        if workspace_root.exists():
            shutil.rmtree(workspace_root)
        shutil.copytree(workspace_source, workspace_root)
        workspace_before = _snapshot_workspace(workspace_root)
        _write_json(output_dir / "workspace_before.json", workspace_before)

    sink_cfg = config_data["result_sink"]
    sink_server, sink_state, _ = start_result_sink(
        sink_cfg["bind_host"],
        int(sink_cfg.get("port", 0)),
        sink_cfg["path"],
    )

    source_cfg = config_data["source_fixture"]
    source_server, source_state, _ = start_source_fixture(
        source_cfg["bind_host"],
        int(source_cfg.get("port", 0)),
        source_cfg["path"],
        source_content,
        pages=source_pages,
    )

    x_server = None
    x_state = None
    x_urls = None
    x_cfg = config_data.get("x_fixture")
    x_asset = case_data.get("fixture", {}).get("x_asset")
    x_fixture_data = {}
    if x_asset:
        x_fixture_data = _load_json(case_dir / x_asset)
    if x_cfg:
        x_server, x_state, x_urls = start_x_fixture(
            x_cfg["bind_host"],
            int(x_cfg.get("port", 0)),
            x_cfg["post_path"],
            x_cfg["search_path"],
            x_cfg["user_posts_path"],
            x_fixture_data,
        )

    model_server = None
    model_state = None
    model_agent_url = None
    model_error_agent_url = None
    model_cfg = config_data.get("model_fixture")
    if model_cfg:
        model_server, model_state, model_urls = start_model_gateway(
            model_cfg["bind_host"],
            int(model_cfg.get("port", 0)),
            model_cfg["path"],
            model_cfg.get("error_path", model_cfg["path"] + "-error"),
            backend=model_cfg.get("backend", "mock"),
            model=model_cfg.get("model", "gpt-5.4-mini"),
            api_key_env=model_cfg.get("api_key_env", "OPENAI_API_KEY"),
        )
        model_agent_url = "http://{host}:{port}{path}".format(
            host=model_cfg["agent_host"],
            port=model_server.server.server_port,
            path=model_cfg["path"],
        )
        model_error_agent_url = "http://{host}:{port}{path}".format(
            host=model_cfg["agent_host"],
            port=model_server.server.server_port,
            path=model_cfg.get("error_path", model_cfg["path"] + "-error"),
        )

    interpretation_server = None
    interpretation_state = None
    interpretation_agent_url = None
    interpretation_error_agent_url = None
    interpretation_cfg = (
        config_data.get("interpretation_fixture") or config_data.get("translation_fixture")
    )
    if interpretation_cfg:
        interpretation_server, interpretation_state, _ = start_interpretation_gateway(
            interpretation_cfg["bind_host"],
            int(interpretation_cfg.get("port", 0)),
            interpretation_cfg["path"],
            interpretation_cfg.get("error_path", interpretation_cfg["path"] + "-error"),
            backend=interpretation_cfg.get("backend", "mock"),
            model=interpretation_cfg.get("model", "gpt-5.4-mini"),
            api_key_env=interpretation_cfg.get("api_key_env", "OPENAI_API_KEY"),
        )
        interpretation_agent_url = "http://{host}:{port}{path}".format(
            host=interpretation_cfg["agent_host"],
            port=interpretation_server.server.server_port,
            path=interpretation_cfg["path"],
        )
        interpretation_error_agent_url = "http://{host}:{port}{path}".format(
            host=interpretation_cfg["agent_host"],
            port=interpretation_server.server.server_port,
            path=interpretation_cfg.get("error_path", interpretation_cfg["path"] + "-error"),
        )

    bridge_proc = None
    bridge_log = None
    bridge_cfg = config_data.get("m5_bridge")
    if bridge_cfg and workspace_root is not None:
        bridge_bind_host = str(bridge_cfg.get("bind_host", "127.0.0.1"))
        bridge_port = int(bridge_cfg.get("port", 8090))
        bridge_launch_env = _launch_env(config_data)
        host_search_env = str(config_data.get("host_search_key_env", "")).strip()
        if host_search_env:
            host_search_value = _shell_env_value(host_search_env)
            if host_search_value:
                bridge_launch_env[host_search_env] = host_search_value
        bridge_log = open(output_dir / "m5_bridge.log", "wb")
        bridge_command = [
            bridge_launch_env.get("PYTHON", "python3"),
            str(root / "tools" / "m5_host_bridge.py"),
            "--workspace",
            str(workspace_root),
            "--bind",
            bridge_bind_host,
            "--port",
            str(bridge_port),
            "--python-image",
            config_data.get("docker_image", "python:3.12-slim"),
            "--output-dir",
            str(output_dir),
        ]
        bridge_proc = subprocess.Popen(
            bridge_command,
            cwd=root,
            env=bridge_launch_env,
            stdout=bridge_log,
            stderr=subprocess.STDOUT,
            text=False,
            bufsize=0,
        )
        _wait_for_http_ok(f"http://{bridge_bind_host}:{bridge_port}/healthz", 10.0)

    sink_agent_url = "http://{host}:{port}{path}".format(
        host=sink_cfg["agent_host"],
        port=sink_server.server.server_port,
        path=sink_cfg["path"],
    )
    source_agent_url = "http://{host}:{port}{path}".format(
        host=source_cfg["agent_host"],
        port=source_server.server.server_port,
        path=source_cfg["path"],
    )
    source_base_url = "http://{host}:{port}".format(
        host=source_cfg["agent_host"],
        port=source_server.server.server_port,
    )

    replacements = {
        "RESULT_SINK_URL": sink_agent_url,
        "SOURCE_URL": source_agent_url,
    }
    if model_agent_url:
        replacements["MODEL_URL"] = model_agent_url
    if model_error_agent_url:
        replacements["MODEL_ERROR_URL"] = model_error_agent_url
    if interpretation_agent_url:
        replacements["INTERPRETATION_URL"] = interpretation_agent_url
        replacements["TRANSLATION_URL"] = interpretation_agent_url
    if interpretation_error_agent_url:
        replacements["INTERPRETATION_ERROR_URL"] = interpretation_error_agent_url
        replacements["TRANSLATION_ERROR_URL"] = interpretation_error_agent_url
    if x_urls:
        replacements["X_POST_TWEET_URL"] = "http://{host}:{port}{path}".format(
            host=x_cfg["agent_host"],
            port=x_server.server.server_port,
            path=x_cfg["post_path"],
        )
        replacements["X_SEARCH_RECENT_URL"] = "http://{host}:{port}{path}".format(
            host=x_cfg["agent_host"],
            port=x_server.server.server_port,
            path=x_cfg["search_path"],
        )
        replacements["X_USER_POSTS_URL"] = "http://{host}:{port}{path}".format(
            host=x_cfg["agent_host"],
            port=x_server.server.server_port,
            path=x_cfg["user_posts_path"],
        )

    case_data = substitute_placeholders(case_data, replacements)
    turns = build_turns(case_data, replacements)
    input_lines = [build_input_line(turn, replacements) for turn in turns]

    run_metadata = {
        "case": str(case_path.relative_to(root)),
        "config": str(config_path.relative_to(root)),
        "source_url": source_agent_url,
        "source_base_url": source_base_url,
        "result_sink_url": sink_agent_url,
        "model_url": model_agent_url,
        "model_error_url": model_error_agent_url,
        "interpretation_url": interpretation_agent_url,
        "interpretation_error_url": interpretation_error_agent_url,
        "translation_url": interpretation_agent_url,
        "translation_error_url": interpretation_error_agent_url,
        "x_post_tweet_url": replacements.get("X_POST_TWEET_URL"),
        "x_search_recent_url": replacements.get("X_SEARCH_RECENT_URL"),
        "x_user_posts_url": replacements.get("X_USER_POSTS_URL"),
        "agent_command": config_data["agent_command"],
        "path_prefixes": config_data.get("path_prefixes", []),
        "input_lines": input_lines,
        "workspace_root": str(workspace_root) if workspace_root else None,
        "search_fixture_path": str(search_fixture_path) if search_fixture_path else None,
    }
    host_search_env = str(config_data.get("host_search_key_env", "")).strip()
    if host_search_env:
        run_metadata["host_search_secret_env"] = host_search_env
    pre_input_lines: list[str] = []
    pre_input_lines.append("status plain")
    if case_data.get("expect", {}).get("required_trace_events"):
        pre_input_lines.append("trace on")
    if guest_openai_env:
        env_name = str(guest_openai_env)
        env_value = _shell_env_value(env_name)
        if not env_value:
            raise ValueError(
                f"missing required environment variable for guest secret: {env_name}"
            )
        pre_input_lines.append("openai-key " + env_value)
        run_metadata["guest_secret_env"] = env_name
    _write_json(output_dir / "run.json", run_metadata)

    command = config_data["agent_command"]
    workdir = root / config_data.get("workdir", ".")
    launch_env = _launch_env(config_data)
    for key, value in replacements.items():
        launch_env[f"HARNESS_{key}"] = value
    launch_env["HARNESS_SOURCE_BASE_URL"] = source_base_url
    launch_env["HARNESS_OUTPUT_DIR"] = str(output_dir)
    launch_env["HARNESS_DOCKER_IMAGE"] = config_data.get("docker_image", "python:3.12-slim")
    if workspace_root is not None:
        launch_env["HARNESS_WORKSPACE_ROOT"] = str(workspace_root)
    if search_fixture_path is not None:
        launch_env["HARNESS_SEARCH_FIXTURE_PATH"] = str(search_fixture_path)
    proc = subprocess.Popen(
        command,
        cwd=workdir,
        env=launch_env,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=False,
        bufsize=0,
    )

    out_queue: queue.Queue[str | None] = queue.Queue()
    log_chunks: list[str] = []
    raw_chunks: list[bytes] = []
    reader = threading.Thread(
        target=_reader, args=(proc.stdout, out_queue, log_chunks, raw_chunks), daemon=True
    )
    reader.start()

    trace_events = []
    trace_lines = []
    pending_text = ""
    full_text = ""
    prompt_seen = False
    prompt = case_data["expect"].get("boot_prompt", config_data.get("prompt", "Goal >"))
    boot_deadline = time.time() + int(config_data.get("boot_timeout_sec", 10))
    prompt_count = 0
    session_transcript = []
    guest_exception_line = None

    def process_output(item: str):
        nonlocal pending_text, full_text, prompt_count, guest_exception_line
        full_text += item
        pending_text += item
        prompt_count = full_text.count(prompt)
        if guest_exception_line is None:
            guest_exception_line = _detect_guest_exception(full_text)
        while "\n" in pending_text:
            line, pending_text = pending_text.split("\n", 1)
            marker = line.find(TRACE_PREFIX)
            if marker == -1:
                continue
            raw_json = line[marker + len(TRACE_PREFIX) :].strip()
            if not raw_json:
                continue
            trace_lines.append(raw_json)
            try:
                trace_events.append(json.loads(raw_json))
            except json.JSONDecodeError:
                pass

    try:
        while time.time() < boot_deadline:
            try:
                item = out_queue.get(timeout=0.2)
            except queue.Empty:
                if proc.poll() is not None:
                    break
                continue
            if item is None:
                break
            process_output(item)
            if prompt in full_text:
                prompt_seen = True
                break
        prelude_timeout_sec = int(config_data.get("prelude_timeout_sec", 8))
        if prompt_seen and proc.stdin is not None:
            for prelude in pre_input_lines:
                prelude_prompt_count = prompt_count
                proc.stdin.write((prelude + "\n").encode("utf-8"))
                proc.stdin.flush()
                prelude_deadline = time.time() + prelude_timeout_sec
                while time.time() < prelude_deadline:
                    if prompt_count > prelude_prompt_count:
                        break
                    try:
                        item = out_queue.get(timeout=0.2)
                    except queue.Empty:
                        if proc.poll() is not None:
                            break
                        continue
                    if item is None:
                        break
                    process_output(item)

        completion_events = set(
            case_data.get(
                "turn_completion_trace_events",
                [
                    "goal_completed",
                    "goal_refused",
                    "goal_failed",
                    "assistant_response_rendered",
                    "loop_stopped",
                ],
            )
        )
        settle_after_result_sec = 0.5
        settle_after_completion_sec = 0.5
        settle_after_exception_sec = 1.0
        for turn_index, turn in enumerate(turns, start=1):
            if proc.stdin is None:
                break
            input_line = input_lines[turn_index - 1]
            turn_prompt_count = prompt_count
            turn_text_start = len(full_text)
            turn_trace_start = len(trace_events)
            turn_sink_start = len(sink_state.requests)
            turn_source_start = len(source_state.requests)
            turn_model_start = len(model_state.requests) if model_state is not None else 0
            turn_interpretation_start = (
                len(interpretation_state.requests)
                if interpretation_state is not None
                else 0
            )
            turn_x_post_start = len(x_state.post_requests) if x_state is not None else 0
            turn_x_search_start = len(x_state.search_requests) if x_state is not None else 0
            turn_x_user_start = (
                len(x_state.user_posts_requests) if x_state is not None else 0
            )

            proc.stdin.write((input_line + "\n").encode("utf-8"))
            proc.stdin.flush()

            turn_deadline = time.time() + int(
                turn.get("expect", {}).get(
                    "timeout_sec",
                    case_data["expect"].get(
                        "timeout_sec", config_data.get("case_timeout_sec", 30)
                    ),
                )
            )
            result_seen_at = None
            completion_seen_at = None
            exception_seen_at = None
            while time.time() < turn_deadline:
                if prompt_count > turn_prompt_count:
                    break
                try:
                    item = out_queue.get(timeout=0.2)
                except queue.Empty:
                    if len(sink_state.requests) > turn_sink_start and result_seen_at is None:
                        result_seen_at = time.time()
                    turn_trace_names = {
                        event.get("event")
                        for event in trace_events[turn_trace_start:]
                        if isinstance(event, dict)
                    }
                    if turn_trace_names.intersection(completion_events) and completion_seen_at is None:
                        completion_seen_at = time.time()
                    if result_seen_at is not None and (
                        time.time() - result_seen_at >= settle_after_result_sec
                    ):
                        break
                    if guest_exception_line is not None and exception_seen_at is None:
                        exception_seen_at = time.time()
                    if exception_seen_at is not None and (
                        time.time() - exception_seen_at >= settle_after_exception_sec
                    ):
                        break
                    if completion_seen_at is not None and (
                        time.time() - completion_seen_at >= settle_after_completion_sec
                    ):
                        break
                    if proc.poll() is not None and (
                        result_seen_at is not None
                        or completion_seen_at is not None
                        or guest_exception_line is not None
                    ):
                        break
                    continue
                if item is None:
                    break
                process_output(item)
                if guest_exception_line is not None and exception_seen_at is None:
                    exception_seen_at = time.time()
                if len(sink_state.requests) > turn_sink_start and result_seen_at is None:
                    result_seen_at = time.time()
                turn_trace_names = {
                    event.get("event")
                    for event in trace_events[turn_trace_start:]
                    if isinstance(event, dict)
                }
                if turn_trace_names.intersection(completion_events) and completion_seen_at is None:
                    completion_seen_at = time.time()
            if guest_exception_line is not None and completion_seen_at is None:
                completion_seen_at = time.time()

            turn_text = full_text[turn_text_start:]
            turn_trace = trace_events[turn_trace_start:]
            turn_result_payload = None
            if len(sink_state.requests) > turn_sink_start:
                turn_result_payload = decode_result_payload(sink_state.requests[-1])
            turn_terminal_result = _extract_terminal_result(turn_text, turn_trace)
            turn_intent_ir = _extract_intent_ir(turn_trace)
            turn_tool_calls = _extract_tool_calls(turn_trace)
            session_transcript.append(
                {
                    "turn": turn_index,
                    "input_line": input_line,
                    "trace_events": [event.get("event") for event in turn_trace if isinstance(event, dict)],
                    "terminal_result": turn_terminal_result,
                    "intent_ir": turn_intent_ir,
                    "tool_calls": turn_tool_calls,
                    "result_payload": turn_result_payload,
                    "observations": {
                        "sink_requests": len(sink_state.requests) - turn_sink_start,
                        "source_requests": len(source_state.requests) - turn_source_start,
                        "model_requests": (
                            (len(model_state.requests) if model_state is not None else 0)
                            - turn_model_start
                        ),
                        "interpretation_requests": (
                            (
                                len(interpretation_state.requests)
                                if interpretation_state is not None
                                else 0
                            )
                            - turn_interpretation_start
                        ),
                        "x_post_requests": (
                            (len(x_state.post_requests) if x_state is not None else 0)
                            - turn_x_post_start
                        ),
                        "x_search_requests": (
                            (len(x_state.search_requests) if x_state is not None else 0)
                            - turn_x_search_start
                        ),
                        "x_user_posts_requests": (
                            (
                                len(x_state.user_posts_requests)
                                if x_state is not None
                                else 0
                            )
                            - turn_x_user_start
                        ),
                    },
                }
            )
    finally:
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait(timeout=2)
        sink_server.stop()
        source_server.stop()
        if model_server is not None:
            model_server.stop()
        if interpretation_server is not None:
            interpretation_server.stop()
        if x_server is not None:
            x_server.stop()
        if bridge_proc is not None:
            if bridge_proc.poll() is None:
                bridge_proc.terminate()
                try:
                    bridge_proc.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    bridge_proc.kill()
                    bridge_proc.wait(timeout=2)
            if bridge_log is not None:
                bridge_log.close()

    uart_raw = b"".join(raw_chunks)
    uart_text = "".join(log_chunks)
    redact_values: list[str] = []
    guest_secret_env = run_metadata.get("guest_secret_env")
    if guest_secret_env:
        env_value = _shell_env_value(str(guest_secret_env))
        if env_value:
            redact_values.append(env_value)
            redact_values.append("openai-key " + env_value)
    uart_text, uart_raw = _redact_uart_artifacts(uart_text, uart_raw, redact_values)
    (output_dir / "uart.raw.log").write_bytes(uart_raw)
    (output_dir / "uart.log").write_text(uart_text, encoding="utf-8")
    trace_lines = []
    trace_events = []
    for raw_line in uart_text.splitlines():
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
            pass
    trace_path = output_dir / "trace.jsonl"
    trace_path.write_text(
        "".join(line + "\n" for line in trace_lines),
        encoding="utf-8",
    )
    llm_api_log = extract_llm_api_log(trace_events)
    if llm_api_log:
        write_llm_api_log_jsonl(output_dir / "llm_api_log.jsonl", llm_api_log)

    result_payload = None
    if sink_state.requests:
        result_payload = decode_result_payload(sink_state.requests[-1])
        _write_json(output_dir / "result.json", result_payload)

    terminal_result = _extract_terminal_result(uart_text, trace_events)
    if terminal_result is None and guest_exception_line is not None:
        terminal_result = {
            "status": "error",
            "reason": guest_exception_line,
        }
    if terminal_result is not None:
        _write_json(output_dir / "terminal_result.json", terminal_result)

    intent_ir = _extract_intent_ir(trace_events)
    if intent_ir is not None:
        _write_json(output_dir / "intent_ir.json", intent_ir)

    tool_calls = _extract_tool_calls(trace_events)
    if tool_calls:
        _write_json(output_dir / "tool_calls.json", tool_calls)
    if session_transcript:
        _write_json(output_dir / "session_transcript.json", session_transcript)

    workspace_after = None
    if workspace_root is not None:
        workspace_after = _snapshot_workspace(workspace_root)
        _write_json(output_dir / "workspace_after.json", workspace_after)

    process_runs = []
    process_runs_path = output_dir / "process_runs.json"
    if process_runs_path.exists():
        process_runs = _load_json(process_runs_path)
    tool_errors = []
    tool_errors_path = output_dir / "tool_errors.json"
    if tool_errors_path.exists():
        tool_errors = _load_json(tool_errors_path)
    search_results = None
    search_results_path = output_dir / "search_results.json"
    if search_results_path.exists():
        search_results = _load_json(search_results_path)
    fetched_sources = None
    fetched_sources_path = output_dir / "fetched_sources.json"
    if fetched_sources_path.exists():
        fetched_sources = _load_json(fetched_sources_path)
    elif session_transcript:
        fetched_sources = _synthesize_fetched_sources(session_transcript, search_results)
        if fetched_sources is not None:
            _write_json(fetched_sources_path, fetched_sources)
    source_memory = None
    source_memory_path = output_dir / "source_memory.json"
    if source_memory_path.exists():
        source_memory = _load_json(source_memory_path)
    memory_snapshot = None
    memory_snapshot_path = output_dir / "memory_snapshot.json"
    if memory_snapshot_path.exists():
        memory_snapshot = _load_json(memory_snapshot_path)
    else:
        memory_snapshot = _extract_memory_snapshot(trace_events)
        if memory_snapshot is not None:
            _write_json(memory_snapshot_path, memory_snapshot)
    memory_events = None
    memory_events_path = output_dir / "memory_events.json"
    if memory_events_path.exists():
        memory_events = _load_json(memory_events_path)
    else:
        memory_events = _extract_memory_events(trace_events)
        if memory_events is not None:
            _write_json(memory_events_path, memory_events)
    context_snapshot = None
    context_snapshot_path = output_dir / "context_snapshot.json"
    if context_snapshot_path.exists():
        context_snapshot = _load_json(context_snapshot_path)
    else:
        context_snapshot = _extract_context_snapshot(trace_events)
        if context_snapshot is not None:
            _write_json(context_snapshot_path, context_snapshot)
    context_budget = None
    context_budget_path = output_dir / "context_budget.json"
    if context_budget_path.exists():
        context_budget = _load_json(context_budget_path)
    else:
        context_budget = _extract_context_budget(trace_events)
        if context_budget is not None:
            _write_json(context_budget_path, context_budget)
    checkpoint_snapshot = None
    checkpoint_snapshot_path = output_dir / "checkpoint_snapshot.json"
    if checkpoint_snapshot_path.exists():
        checkpoint_snapshot = _load_json(checkpoint_snapshot_path)

    observations = {
        "sink_requests": len(sink_state.requests),
        "source_requests": len(source_state.requests),
        "model_requests": len(model_state.requests) if model_state is not None else 0,
        "interpretation_requests": (
            len(interpretation_state.requests) if interpretation_state is not None else 0
        ),
        "translation_requests": (
            len(interpretation_state.requests) if interpretation_state is not None else 0
        ),
        "x_post_requests": len(x_state.post_requests) if x_state is not None else 0,
        "x_search_requests": len(x_state.search_requests) if x_state is not None else 0,
        "x_user_posts_requests": (
            len(x_state.user_posts_requests) if x_state is not None else 0
        ),
        "guest_exception": guest_exception_line,
    }
    _write_json(output_dir / "observations.json", observations)

    report = evaluate_case(
        case_data,
        trace_events,
        result_payload,
        prompt_seen,
        observations=observations,
        uart_text=uart_text,
        terminal_result=terminal_result,
        intent_ir=intent_ir,
        session_transcript=session_transcript,
        tool_calls=tool_calls,
        workspace_before=workspace_before,
        workspace_after=workspace_after,
        process_runs=process_runs,
        tool_errors=tool_errors,
        search_results=search_results,
        fetched_sources=fetched_sources,
        source_memory=source_memory,
        memory_snapshot=memory_snapshot,
        memory_events=memory_events,
        context_snapshot=context_snapshot,
        context_budget=context_budget,
        checkpoint_snapshot=checkpoint_snapshot,
    )
    _write_json(output_dir / "report.json", report)
    return 0 if report["pass"] else 1


def main():
    parser = argparse.ArgumentParser(description="Run a MiniAgentOS harness case")
    parser.add_argument("case", help="Path to the case task.json file")
    parser.add_argument(
        "--config",
        default="harness/config.fixture.json",
        help="Path to the harness config JSON file",
    )
    parser.add_argument(
        "--output",
        help="Output directory. Defaults to output/<case-id>",
    )
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[2]
    case_path = (root / args.case).resolve() if not Path(args.case).is_absolute() else Path(args.case)
    config_path = (root / args.config).resolve() if not Path(args.config).is_absolute() else Path(args.config)
    case_data = _load_json(case_path)
    output_dir = (
        (root / args.output).resolve()
        if args.output
        else (root / "output" / case_data["goal_id"]).resolve()
    )
    raise SystemExit(run_case(case_path, config_path, output_dir))


if __name__ == "__main__":
    main()
