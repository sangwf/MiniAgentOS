#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import sys
import time
import urllib.parse
import urllib.request

REPO_ROOT = Path(__file__).resolve().parents[2]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from harness.lib.m5_substrate import M5Substrate
from harness.lib.m6_substrate import M6Substrate
from harness.lib.m7_substrate import M7Substrate


WORKSPACE_ROOT = Path(os.environ["HARNESS_WORKSPACE_ROOT"]).resolve() if os.environ.get("HARNESS_WORKSPACE_ROOT") else None
OUTPUT_DIR = Path(os.environ["HARNESS_OUTPUT_DIR"]).resolve() if os.environ.get("HARNESS_OUTPUT_DIR") else None
DOCKER_IMAGE = os.environ.get("HARNESS_DOCKER_IMAGE", "python:3.12-slim")
SEARCH_FIXTURE_PATH = (
    Path(os.environ["HARNESS_SEARCH_FIXTURE_PATH"]).resolve()
    if os.environ.get("HARNESS_SEARCH_FIXTURE_PATH")
    else None
)
SOURCE_BASE_URL = os.environ.get("HARNESS_SOURCE_BASE_URL")
M5 = M5Substrate(WORKSPACE_ROOT, OUTPUT_DIR, DOCKER_IMAGE)
M6 = M6Substrate(SEARCH_FIXTURE_PATH, SOURCE_BASE_URL, OUTPUT_DIR)
M7 = M7Substrate(OUTPUT_DIR)


def emit(event, **extra):
    payload = {"event": event, "ts_ms": int(time.time() * 1000)}
    payload.update(extra)
    sys.stdout.write("TRACE " + json.dumps(payload, ensure_ascii=True) + "\n")
    sys.stdout.flush()


def summarize_text(text: str, sentence_limit: int) -> str:
    normalized = " ".join(text.replace("\n", " ").split())
    sentences = []
    for chunk in normalized.split("."):
        chunk = chunk.strip()
        if chunk:
            sentences.append(chunk + ".")
        if len(sentences) >= sentence_limit:
            break
    return " ".join(sentences)


def fetch_text(url: str) -> str:
    with urllib.request.urlopen(url, timeout=10) as response:
        return response.read().decode("utf-8")


def fetch_json(url: str) -> dict:
    with urllib.request.urlopen(url, timeout=10) as response:
        return json.loads(response.read().decode("utf-8"))


def post_json(url: str, payload: dict) -> dict:
    body = json.dumps(payload, ensure_ascii=True).encode("utf-8")
    request = urllib.request.Request(
        url,
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=10) as response:
        body = response.read().decode("utf-8")
        if not body.strip():
            return {}
        return json.loads(body)


def print_prompt(prompt: str):
    sys.stdout.write(prompt + "\n")
    sys.stdout.flush()


def print_reason(prompt: str, reason: str):
    sys.stdout.write("\nreason: " + reason + "\n")
    sys.stdout.flush()
    print_prompt(prompt)


def print_summary(prompt: str, lines: list[str]):
    sys.stdout.write("\n--- summary ---\n")
    for line in lines:
        sys.stdout.write(line + "\n")
    sys.stdout.flush()
    print_prompt(prompt)


def handle_legacy_task(task: dict):
    goal_id = task["goal_id"]
    source_url = task["source_url"]
    sink_url = task["sink_url"]
    sentence_limit = int(task.get("summary_sentences", 3))

    emit("goal_received", step=0, detail={"goal_id": goal_id})
    emit("intent_parsed", step=0, detail={"kind": task.get("kind")})
    emit(
        "plan_created",
        step=0,
        detail={
            "skills": ["fetch_url", "summarize_text", "post_result"],
            "max_steps": task.get("constraints", {}).get("max_steps"),
        },
    )

    emit("skill_called", step=1, skill="fetch_url", detail={"url": source_url})
    source_text = fetch_text(source_url)
    emit("skill_result", step=1, skill="fetch_url", status="ok")

    emit("skill_called", step=2, skill="summarize_text")
    summary = summarize_text(source_text, sentence_limit)
    emit("skill_result", step=2, skill="summarize_text", status="ok")

    emit("skill_called", step=3, skill="post_result", detail={"url": sink_url})
    result = {
        "goal_id": goal_id,
        "status": "ok",
        "summary": summary,
    }
    post_json(sink_url, result)
    emit("skill_result", step=3, skill="post_result", status="ok")

    emit("goal_completed", step=3, status="ok")
    sys.stdout.write("done\n")
    sys.stdout.flush()


def extract_url(text: str) -> str | None:
    match = re.search(r"https?://\S+", text)
    if not match:
        return None
    return match.group(0).rstrip(".,;)")


def extract_username(text: str) -> str | None:
    lowered = text.lower()
    match = re.search(r"what did ([a-z0-9_]+) post recently", lowered)
    if match:
        return match.group(1)
    match = re.search(r"get recent posts from ([a-z0-9_]+)", lowered)
    if match:
        return match.group(1)
    return None


def extract_query(text: str) -> str:
    lowered = text.lower()
    for marker in ("about ", "for ", ": "):
        index = lowered.find(marker)
        if index != -1:
            return text[index + len(marker) :].strip().rstrip(".")
    return text.strip().rstrip(".")


def extract_post_request(text: str) -> tuple[str, dict] | None:
    match = re.search(r"post\s+(?P<body>\{.*?\})\s+to\s+(?P<url>https?://\S+)", text, re.I)
    if not match:
        return None
    url = match.group("url").rstrip(".,;)")
    try:
        body = json.loads(match.group("body"))
    except json.JSONDecodeError:
        return None
    if not isinstance(body, dict):
        return None
    return url, body


def render_post_lines(posts: list[dict]) -> list[str]:
    lines = []
    for post in posts[:3]:
        author = post.get("author", "unknown")
        lines.append(f"- @{author}: {post.get('text', '')}")
    return lines or ["- No posts found."]


def tool_call(tool: str, arguments: dict, result: dict | None = None, status: str = "ok"):
    emit("tool_call_requested", tool=tool, arguments=arguments)
    emit("tool_call_started", tool=tool, arguments=arguments)
    emit("tool_call_completed", tool=tool, arguments=arguments, status=status, result=result or {})
    return {"status": status, "result": result or {}}


def _find_function_name(content: str) -> str | None:
    for line in content.splitlines():
        stripped = line.strip()
        if stripped.startswith("def "):
            return stripped.split("def ", 1)[1].split("(", 1)[0]
    return None


def _extract_search_query(text: str) -> str | None:
    patterns = [
        r"search the web for (?P<query>.+?)(?: and |\.|$)",
        r"search for (?P<query>.+?)(?: and |\.|$)",
    ]
    for pattern in patterns:
        match = re.search(pattern, text, re.I)
        if not match:
            continue
        query = match.group("query").strip()
        if query:
            return query
    return None


def _extract_labeled_value(content: str, label: str) -> str | None:
    prefix = label.lower() + ":"
    for line in content.splitlines():
        stripped = line.strip()
        if stripped.lower().startswith(prefix):
            return stripped.split(":", 1)[1].strip()
    return None


def _fetch_result_and_record(result: dict, turn_index: int, tool_index: int) -> str:
    content = fetch_text(result["url"])
    M6.record_fetch(
        url=result["url"],
        content=content,
        turn_index=turn_index,
        tool_call_index=tool_index,
        search_result_id=result.get("id"),
    )
    return content


def _answer_from_content(content: str) -> str:
    for label in ("Answer", "Provider", "Score", "Summary"):
        value = _extract_labeled_value(content, label)
        if value:
            return value
    summary = summarize_text(content, 2)
    return summary or "I read the selected source but could not extract a concise answer."


def _comparison_line(contents: list[tuple[str, str]]) -> str:
    lines = []
    for title, content in contents:
        value = _extract_labeled_value(content, "Position") or _extract_labeled_value(
            content, "Summary"
        ) or summarize_text(content, 1)
        lines.append(f"{title}: {value}")
    return "; ".join(lines)


def _performance_line(records: list[dict]) -> str:
    used_ids = []
    claims = []
    for item in records:
        value = _extract_labeled_value(item["content"], "Performance")
        if not value:
            continue
        claims.append(value)
        used_ids.append(item["id"])
    if used_ids:
        M6.mark_used(used_ids, records[0]["turn_index"])
    if claims:
        return "; ".join(claims)
    return "No explicit performance claims were recorded in the known sources."


def _m7_public_state(session_state: dict) -> dict:
    data = {}
    for key, value in session_state.items():
        if not key.startswith("m7_"):
            continue
        if key == "m7_recent_lines":
            continue
        data[key] = value
    return data


def _m7_session_state_text(session_state: dict) -> str:
    public_state = _m7_public_state(session_state)
    if not public_state:
        return ""
    return json.dumps(public_state, ensure_ascii=False, sort_keys=True)


def _m7_recent_conversation_text(session_state: dict) -> str:
    lines = session_state.get("m7_recent_lines", [])
    if not isinstance(lines, list):
        return ""
    rendered = [str(line) for line in lines[-6:]]
    return "\n".join(rendered)


def _m7_append_turn(session_state: dict, user_text: str, assistant_text: str) -> None:
    lines = session_state.setdefault("m7_recent_lines", [])
    if not isinstance(lines, list):
        lines = []
        session_state["m7_recent_lines"] = lines
    lines.append(f"User: {user_text}")
    lines.append(f"Assistant: {assistant_text}")
    while len(lines) > 8:
        del lines[0]


def _m7_update_conversation_memory(session_state: dict, turn_index: int) -> None:
    recent = _m7_recent_conversation_text(session_state)
    if not recent:
        return
    M7.add_memory(
        "conversation",
        "Recent conversation tail retained for follow-up continuity.",
        source="recent_conversation",
        state="derived",
        detail=recent,
        turn_index=turn_index,
        entry_id="mem-conversation",
    )


def _m7_record_context(
    *,
    session_state: dict,
    turn_index: int,
    current_request: str,
    latest_tool_result: str = "",
) -> None:
    M7.record_context_snapshot(
        turn_index=turn_index,
        current_request=current_request,
        latest_tool_result=latest_tool_result,
        session_state=_m7_session_state_text(session_state),
        recent_conversation=_m7_recent_conversation_text(session_state),
        instructions_chars=1820,
    )


def _handle_m7_turn(prompt: str, text: str, session_state: dict, turn_index: int) -> str | None:
    if not M7.available():
        return None

    lowered = text.lower().strip()

    if "inspect memory" in lowered or "show memory" in lowered:
        emit("model_turn_completed", stop_reason="tool_call")
        task_entry = M7.add_memory(
            "task",
            "User requested direct inspection of the current working memory.",
            source="user_turn",
            state="derived",
            turn_index=turn_index,
            entry_id="mem-task-inspect",
        )
        source_entry = M7.add_memory(
            "source",
            "MiniAgentOS M7 memory entries are explicit runtime-owned state.",
            source="fixture:m7-memory",
            state="raw",
            detail="This retained source fact exists so memory inspection can show both task and source layers.",
            turn_index=turn_index,
            entry_id="mem-source-inspect",
        )
        session_state["m7_last_source_id"] = source_entry["entry"]["id"]
        _m7_record_context(session_state=session_state, turn_index=turn_index, current_request=text)
        status = M7.memory_status()
        tool_call("memory_status", {}, status)
        listing = M7.list_memory(limit=10)
        tool_call("list_memory", {"limit": 10}, listing)
        read_result = M7.read_memory(source_entry["entry"]["id"])
        tool_call("read_memory", {"id": source_entry["entry"]["id"]}, read_result)
        answer = "Memory inspection shows one task entry and one source entry retained."
        _m7_append_turn(session_state, text, answer)
        _m7_update_conversation_memory(session_state, turn_index)
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, [f"- {answer}"])
        return "ok"

    if "report context budget" in lowered or "context budget by layer" in lowered:
        emit("model_turn_completed", stop_reason="tool_call")
        M7.add_memory(
            "task",
            "User requested a report of prompt budget usage by layer.",
            source="user_turn",
            state="derived",
            turn_index=turn_index,
            entry_id="mem-task-budget",
        )
        M7.add_memory(
            "execution",
            "Latest tool result contained a medium-sized bounded execution summary.",
            source="run_process:fixture-check",
            state="raw",
            detail="failure: add(2, 3) returned -1 instead of 5; traceback omitted for brevity; bounded execution summary retained.",
            turn_index=turn_index,
            entry_id="mem-exec-budget",
        )
        latest_tool_result = (
            "tool_result: verification output repeated for budgeting. " * 20
        ).strip()
        _m7_record_context(
            session_state=session_state,
            turn_index=turn_index,
            current_request=text,
            latest_tool_result=latest_tool_result,
        )
        status = M7.memory_status()
        tool_call("memory_status", {}, status)
        answer = (
            "Context budget recorded instructions, current request, latest tool result, working memory, known sources, workspace memory, session state, and recent conversation."
        )
        _m7_append_turn(session_state, text, answer)
        _m7_update_conversation_memory(session_state, turn_index)
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, [f"- {answer}"])
        return "ok"

    if "compact the large source memory" in lowered or "compact memory truthfully" in lowered:
        emit("model_turn_completed", stop_reason="tool_call")
        source_entry = M7.add_memory(
            "source",
            "The retained fact is that Project Atlas is the codename mentioned in the long source.",
            source="fetch_url:https://fixture.local/m7/atlas",
            state="raw",
            detail=(
                "Project Atlas is the codename mentioned in the long source. "
                + ("background filler " * 80)
            ).strip(),
            turn_index=turn_index,
            entry_id="mem-source-compact",
        )
        _m7_record_context(
            session_state=session_state,
            turn_index=turn_index,
            current_request=text,
            latest_tool_result=source_entry["entry"]["detail"],
        )
        compact_result = M7.compact_memory([source_entry["entry"]["id"]], turn_index=turn_index)
        tool_call(
            "compact_memory",
            {"ids": [source_entry["entry"]["id"]], "mode": "bounded_summary"},
            compact_result,
        )
        read_result = M7.read_memory(source_entry["entry"]["id"])
        tool_call("read_memory", {"id": source_entry["entry"]["id"]}, read_result)
        answer = "Compacted memory still retains that Project Atlas is the codename, and the entry is now marked compacted."
        _m7_append_turn(session_state, text, answer)
        _m7_update_conversation_memory(session_state, turn_index)
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, [f"- {answer}"])
        return "ok"

    if "remember that m6 uses brave search" in lowered:
        emit("model_turn_completed", stop_reason="tool_call")
        M7.add_memory(
            "task",
            "User wants a remembered research fact about the M6 search provider.",
            source="user_turn",
            state="derived",
            turn_index=turn_index,
            entry_id="mem-task-research",
        )
        source_entry = M7.add_memory(
            "source",
            "M6 uses Brave Search as its bounded search provider.",
            source="fetch_url:https://fixture.local/m6/provider",
            state="raw",
            detail="The fetched provider note says M6 uses Brave Search through BRAVE_API_KEY.",
            turn_index=turn_index,
            entry_id="mem-source-research",
        )
        session_state["m7_research_source_id"] = source_entry["entry"]["id"]
        _m7_record_context(session_state=session_state, turn_index=turn_index, current_request=text)
        listing = M7.list_memory(kind="source", limit=10)
        tool_call("list_memory", {"kind": "source", "limit": 10}, listing)
        answer = "Stored one research source stating that M6 uses Brave Search."
        _m7_append_turn(session_state, text, answer)
        _m7_update_conversation_memory(session_state, turn_index)
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, [f"- {answer}"])
        return "ok"

    if "using remembered research only" in lowered or "what provider did m6 use" in lowered:
        source_id = str(session_state.get("m7_research_source_id", ""))
        if not source_id:
            return None
        emit("model_turn_completed", stop_reason="tool_call")
        _m7_record_context(session_state=session_state, turn_index=turn_index, current_request=text)
        read_result = M7.read_memory(source_id)
        tool_call("read_memory", {"id": source_id}, read_result)
        answer = "The remembered research source says M6 uses Brave Search."
        _m7_append_turn(session_state, text, answer)
        _m7_update_conversation_memory(session_state, turn_index)
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, [f"- {answer}"])
        return "ok"

    if "remember the failing coding result" in lowered:
        emit("model_turn_completed", stop_reason="tool_call")
        M7.add_memory(
            "task",
            "User wants the recent coding failure and its fix to remain available for follow-up.",
            source="user_turn",
            state="derived",
            turn_index=turn_index,
            entry_id="mem-task-coding",
        )
        workspace_entry = M7.add_memory(
            "workspace",
            "app.py defined add(a, b) and originally returned a - b.",
            source="read_file:app.py",
            state="derived",
            detail="The bounded workspace note records the exact faulty line: return a - b",
            turn_index=turn_index,
            entry_id="mem-workspace-coding",
        )
        execution_entry = M7.add_memory(
            "execution",
            "check.py failed because add(2, 3) returned subtraction instead of addition.",
            source="run_process:check.py",
            state="raw",
            detail="Observed failure: expected 5, got -1, then the bounded fix changed add to return a + b.",
            turn_index=turn_index,
            entry_id="mem-exec-coding",
        )
        session_state["m7_workspace_memory_id"] = workspace_entry["entry"]["id"]
        session_state["m7_execution_memory_id"] = execution_entry["entry"]["id"]
        _m7_record_context(session_state=session_state, turn_index=turn_index, current_request=text)
        status = M7.memory_status()
        tool_call("memory_status", {}, status)
        answer = "Stored workspace and execution memory for the bounded coding failure."
        _m7_append_turn(session_state, text, answer)
        _m7_update_conversation_memory(session_state, turn_index)
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, [f"- {answer}"])
        return "ok"

    if "what was the bug" in lowered or "why did add fail" in lowered:
        execution_id = str(session_state.get("m7_execution_memory_id", ""))
        workspace_id = str(session_state.get("m7_workspace_memory_id", ""))
        if not execution_id:
            return None
        emit("model_turn_completed", stop_reason="tool_call")
        _m7_record_context(session_state=session_state, turn_index=turn_index, current_request=text)
        read_exec = M7.read_memory(execution_id)
        tool_call("read_memory", {"id": execution_id}, read_exec)
        if workspace_id:
            read_workspace = M7.read_memory(workspace_id)
            tool_call("read_memory", {"id": workspace_id}, read_workspace)
        answer = "The bug was that add(a, b) returned a - b, so check.py saw subtraction instead of addition."
        _m7_append_turn(session_state, text, answer)
        _m7_update_conversation_memory(session_state, turn_index)
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, [f"- {answer}"])
        return "ok"

    if "save a checkpoint after remembering the provider" in lowered:
        emit("model_turn_completed", stop_reason="tool_call")
        M7.add_memory(
            "task",
            "User wants a checkpoint after retaining the provider fact.",
            source="user_turn",
            state="derived",
            turn_index=turn_index,
            entry_id="mem-task-checkpoint",
        )
        source_entry = M7.add_memory(
            "source",
            "The remembered provider fact is that M6 uses Brave Search.",
            source="fetch_url:https://fixture.local/m6/provider",
            state="raw",
            detail="Checkpoint source fact: M6 uses Brave Search through BRAVE_API_KEY.",
            turn_index=turn_index,
            entry_id="mem-source-checkpoint",
        )
        session_state["m7_checkpoint_source_id"] = source_entry["entry"]["id"]
        _m7_record_context(session_state=session_state, turn_index=turn_index, current_request=text)
        save_result = M7.save_checkpoint("remembered-provider", turn_index=turn_index)
        session_state["m7_checkpoint_id"] = save_result["checkpoint_id"]
        tool_call("save_checkpoint", {"label": "remembered-provider"}, save_result)
        answer = f"Saved checkpoint {save_result['checkpoint_id']} with the remembered provider fact."
        _m7_append_turn(session_state, text, answer)
        _m7_update_conversation_memory(session_state, turn_index)
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, [f"- {answer}"])
        return "ok"

    if "resume from the saved checkpoint" in lowered:
        checkpoint_id = str(session_state.get("m7_checkpoint_id", ""))
        source_id = str(session_state.get("m7_checkpoint_source_id", ""))
        if not checkpoint_id or not source_id:
            return None
        emit("model_turn_completed", stop_reason="tool_call")
        M7.add_memory(
            "conversation",
            "Temporary unrelated memory before resume.",
            source="user_turn",
            state="derived",
            turn_index=turn_index,
            entry_id="mem-temp-before-resume",
        )
        resume_result = M7.resume_checkpoint(checkpoint_id, turn_index=turn_index)
        tool_call("resume_checkpoint", {"checkpoint_id": checkpoint_id}, resume_result)
        _m7_record_context(session_state=session_state, turn_index=turn_index, current_request=text)
        read_result = M7.read_memory(source_id)
        tool_call("read_memory", {"id": source_id}, read_result)
        answer = "Resumed from the saved checkpoint and recovered that M6 uses Brave Search."
        _m7_append_turn(session_state, text, answer)
        _m7_update_conversation_memory(session_state, turn_index)
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, [f"- {answer}"])
        return "ok"

    if "store a large tool result and keep only the key fact" in lowered:
        emit("model_turn_completed", stop_reason="tool_call")
        source_entry = M7.add_memory(
            "source",
            "The retained key fact is 42.",
            source="tool_result:large-fixture",
            state="raw",
            detail=("Key fact: 42. " + ("large result filler " * 120)).strip(),
            turn_index=turn_index,
            entry_id="mem-source-large",
        )
        session_state["m7_large_source_id"] = source_entry["entry"]["id"]
        _m7_record_context(
            session_state=session_state,
            turn_index=turn_index,
            current_request=text,
            latest_tool_result=source_entry["entry"]["detail"],
        )
        compact_result = M7.compact_memory([source_entry["entry"]["id"]], turn_index=turn_index)
        tool_call(
            "compact_memory",
            {"ids": [source_entry["entry"]["id"]], "mode": "bounded_summary"},
            compact_result,
        )
        answer = "Stored the large tool result as compacted memory while retaining the key fact."
        _m7_append_turn(session_state, text, answer)
        _m7_update_conversation_memory(session_state, turn_index)
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, [f"- {answer}"])
        return "ok"

    if "what key fact did you retain" in lowered:
        source_id = str(session_state.get("m7_large_source_id", ""))
        if not source_id:
            return None
        emit("model_turn_completed", stop_reason="tool_call")
        _m7_record_context(session_state=session_state, turn_index=turn_index, current_request=text)
        read_result = M7.read_memory(source_id)
        tool_call("read_memory", {"id": source_id}, read_result)
        answer = "The retained key fact was 42."
        _m7_append_turn(session_state, text, answer)
        _m7_update_conversation_memory(session_state, turn_index)
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, [f"- {answer}"])
        return "ok"

    return None


def _handle_m6_turn(prompt: str, text: str, session_state: dict, turn_index: int) -> str | None:
    if not M6.available():
        return None

    lowered = text.lower().strip()
    tool_index = 0
    search_query = _extract_search_query(text)

    if search_query:
        emit("model_turn_completed", stop_reason="tool_call")
        search_result = M6.search_web(
            search_query,
            turn_index=turn_index,
            tool_call_index=tool_index,
        )
        tool_call(
            "search_web",
            {
                "query": search_query,
                "top_k": 5,
                "freshness": None,
                "domain_allowlist": [],
                "domain_denylist": [],
                "locale": None,
            },
            search_result,
            status="ok" if search_result.get("ok") else "denied",
        )
        tool_index += 1
        if not search_result.get("ok"):
            emit("assistant_response_rendered")
            emit("loop_stopped", stop_reason="policy_denied")
            print_reason(prompt, search_result["error"]["message"])
            return "policy_denied"

        results = search_result.get("results", [])
        session_state["m6_results"] = results
        session_state.setdefault("m6_fetched", {})
        if not results:
            emit("assistant_response_rendered")
            emit("loop_stopped", stop_reason="final_response")
            print_summary(prompt, ["- I could not find enough evidence from search results."])
            return "ok"

        if "compare" in lowered:
            fetched = []
            for result in results[:2]:
                content = _fetch_result_and_record(result, turn_index, tool_index)
                tool_call("fetch_url", {"url": result["url"]}, {"ok": True})
                tool_index += 1
                session_state["m6_fetched"][result["id"]] = {
                    "id": result["id"],
                    "url": result["url"],
                    "title": result["title"],
                    "content": content,
                    "turn_index": turn_index,
                }
                fetched.append((result["title"], content))
            emit("assistant_response_rendered")
            emit("loop_stopped", stop_reason="final_response")
            print_summary(prompt, [f"- Comparison: {_comparison_line(fetched)}"])
            return "ok"

        first = results[0]
        content = _fetch_result_and_record(first, turn_index, tool_index)
        tool_call("fetch_url", {"url": first["url"]}, {"ok": True})
        tool_index += 1
        session_state["m6_fetched"][first["id"]] = {
            "id": first["id"],
            "url": first["url"],
            "title": first["title"],
            "content": content,
            "turn_index": turn_index,
        }
        if "prove" in lowered or "confirm" in lowered:
            evidence = _extract_labeled_value(content, "Evidence")
            verdict = _extract_labeled_value(content, "Verdict")
            if verdict == "insufficient" or not evidence:
                emit("assistant_response_rendered")
                emit("loop_stopped", stop_reason="final_response")
                print_summary(prompt, ["- I cannot confirm that claim from the fetched evidence."])
                return "ok"
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, [f"- {_answer_from_content(content)}"])
        return "ok"

    fetched_values = list(session_state.get("m6_fetched", {}).values())
    if fetched_values and "performance" in lowered:
        emit("model_turn_completed", stop_reason="final_response")
        line = _performance_line(
            [
                {
                    "id": item["id"],
                    "content": item["content"],
                    "turn_index": turn_index,
                }
                for item in fetched_values
            ]
        )
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, [f"- {line}"])
        return "ok"

    return None


def _handle_m5_turn(prompt: str, text: str) -> str | None:
    if not M5.available():
        return None
    lowered = text.lower().strip()

    if "which function returns the greeting string" in lowered:
        emit("model_turn_completed", stop_reason="tool_call")
        listing = M5.list_workspace("", depth=3)
        tool_call("list_workspace", {"path": "", "depth": 3}, listing)
        readme = M5.read_file("README.md")
        tool_call("read_file", {"path": "README.md", "offset": 0, "limit": 4096}, readme)
        app = M5.read_file("src/app.py")
        tool_call("read_file", {"path": "src/app.py", "offset": 0, "limit": 4096}, app)
        function_name = _find_function_name(app["content"]) or "unknown"
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, [f"- The `{function_name}` function returns the greeting string."])
        return "ok"

    if "apply a patch to fix the typo" in lowered:
        emit("model_turn_completed", stop_reason="tool_call")
        file_result = M5.read_file("main.py")
        tool_call("read_file", {"path": "main.py", "offset": 0, "limit": 4096}, file_result)
        patch = (
            "*** Begin Patch\n"
            "*** Update File: main.py\n"
            "@@\n"
            '-print("helo")\n'
            '+print("hello")\n'
            "*** End Patch\n"
        )
        patch_result = M5.apply_patch(patch)
        tool_call("apply_patch", {"patch": patch}, patch_result)
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, ["- Applied a patch to `main.py` so it now prints `hello`."])
        return "ok"

    if "fix the typo in the output string" in lowered:
        emit("model_turn_completed", stop_reason="tool_call")
        file_result = M5.read_file("main.py")
        tool_call("read_file", {"path": "main.py", "offset": 0, "limit": 4096}, file_result)
        updated = file_result["content"].replace("helo", "hello")
        write_result = M5.write_file("main.py", updated)
        tool_call("write_file", {"path": "main.py", "create": True, "overwrite": True}, write_result)
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, ["- Fixed the typo in `main.py` so it now prints `hello`."])
        return "ok"

    if "run the main python program" in lowered:
        emit("model_turn_completed", stop_reason="tool_call")
        run_result = M5.run_process(["python3", "main.py"], profile="default")
        tool_call("run_process", {"argv": ["python3", "main.py"], "cwd": "", "profile": "default", "timeout_sec": 20}, run_result)
        process_id = run_result["process_id"]
        output = M5.read_process_output(process_id)
        tool_call("read_process_output", {"process_id": process_id, "offset": 0, "limit": 8192}, output)
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        stdout = output["stdout"].strip() or "(no output)"
        print_summary(prompt, [f"- `main.py` prints: {stdout}"])
        return "ok"

    if "run inline python with -c" in lowered:
        emit("model_turn_completed", stop_reason="tool_call")
        run_result = M5.run_process(["python3", "-c", "print(42)"], profile="default")
        status = "denied" if not run_result.get("ok") else "ok"
        tool_call(
            "run_process",
            {"argv": ["python3", "-c", "print(42)"], "cwd": "", "profile": "default", "timeout_sec": 20},
            run_result,
            status=status,
        )
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="policy_denied")
        print_reason(prompt, run_result["error"]["message"])
        return "policy_denied"

    if "read ../outside.txt" in lowered:
        emit("model_turn_completed", stop_reason="tool_call")
        read_result = M5.read_file("../outside.txt")
        status = "denied" if not read_result.get("ok") else "ok"
        tool_call(
            "read_file",
            {"path": "../outside.txt", "offset": 0, "limit": 4096},
            read_result,
            status=status,
        )
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="policy_denied")
        print_reason(prompt, read_result["error"]["message"])
        return "policy_denied"

    if "fix the failing regression" in lowered:
        emit("model_turn_completed", stop_reason="tool_call")
        listing = M5.list_workspace("", depth=3)
        tool_call("list_workspace", {"path": "", "depth": 3}, listing)
        app = M5.read_file("app.py")
        tool_call("read_file", {"path": "app.py", "offset": 0, "limit": 4096}, app)
        check = M5.read_file("check.py")
        tool_call("read_file", {"path": "check.py", "offset": 0, "limit": 4096}, check)
        first_run = M5.run_process(["python3", "check.py"], profile="test")
        tool_call("run_process", {"argv": ["python3", "check.py"], "cwd": "", "profile": "test", "timeout_sec": 20}, first_run)
        first_output = M5.read_process_output(first_run["process_id"])
        tool_call("read_process_output", {"process_id": first_run["process_id"], "offset": 0, "limit": 8192}, first_output)
        fixed = app["content"].replace("return a - b", "return a + b")
        write_result = M5.write_file("app.py", fixed)
        tool_call("write_file", {"path": "app.py", "create": True, "overwrite": True}, write_result)
        second_run = M5.run_process(["python3", "check.py"], profile="test")
        tool_call("run_process", {"argv": ["python3", "check.py"], "cwd": "", "profile": "test", "timeout_sec": 20}, second_run)
        second_output = M5.read_process_output(second_run["process_id"])
        tool_call("read_process_output", {"process_id": second_run["process_id"], "offset": 0, "limit": 8192}, second_output)
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, ["- Fixed the regression in `app.py` and the verification script now passes."])
        return "ok"

    return None


def handle_text_turn(prompt: str, text: str, session_state: dict, turn_index: int):
    lowered = text.lower().strip()

    emit("user_turn_received")
    emit("model_turn_started")

    m7_status = _handle_m7_turn(prompt, text, session_state, turn_index)
    if m7_status == "ok":
        emit("goal_completed", status="ok")
        return
    if m7_status == "policy_denied":
        emit("goal_refused", status="refused")
        return

    m6_status = _handle_m6_turn(prompt, text, session_state, turn_index)
    if m6_status == "ok":
        emit("goal_completed", status="ok")
        return
    if m6_status == "policy_denied":
        emit("goal_refused", status="refused")
        return

    m5_status = _handle_m5_turn(prompt, text)
    if m5_status == "ok":
        emit("goal_completed", status="ok")
        return
    if m5_status == "policy_denied":
        emit("goal_refused", status="refused")
        return

    if "forbidden tool" in lowered or "admin tool" in lowered:
        emit("model_turn_completed", stop_reason="tool_call")
        emit(
            "tool_call_requested",
            tool="admin_reset",
            arguments={"scope": "system"},
        )
        emit(
            "tool_call_denied",
            tool="admin_reset",
            arguments={"scope": "system"},
            reason="tool policy denied",
        )
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="policy_denied")
        print_reason(prompt, "tool policy denied")
        return

    if "keep looping" in lowered or "loop budget" in lowered:
        step = 0
        emit("model_turn_completed", stop_reason="tool_call", step=step)
        while step < 5:
            key = f"loop_step_{step}"
            tool_call("read_session_state", {"key": key}, {"value": ""})
            if step < 4:
                step += 1
                emit("model_turn_started", step=step)
                emit("model_turn_completed", stop_reason="tool_call", step=step)
            else:
                break
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="budget_exceeded")
        print_reason(prompt, "m4 loop budget exceeded")
        return

    post_request = extract_post_request(text)
    if post_request:
        url, payload = post_request
        emit("model_turn_completed", stop_reason="tool_call")
        response = post_json(url, payload)
        tool_call(
            "post_url",
            {
                "url": url,
                "json": json.dumps(payload, ensure_ascii=True, separators=(",", ":")),
            },
            response,
        )
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, ["- Posted JSON to the requested endpoint."])
        return

    if lowered.startswith("post a tweet:") or lowered.startswith("tweet "):
        tweet_text = text.split(":", 1)[-1].strip() if ":" in text else text[6:].strip()
        emit("model_turn_completed", stop_reason="tool_call")
        response = post_json(
            os.environ["HARNESS_X_POST_TWEET_URL"],
            {"text": tweet_text},
        )
        tool_call("post_tweet", {"text": tweet_text}, response)
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, [f"- Posted tweet {response.get('tweet_id', 'unknown')}."])
        return

    if "search recent posts" in lowered or "search x" in lowered:
        query = extract_query(text)
        emit("model_turn_completed", stop_reason="tool_call")
        search_url = os.environ["HARNESS_X_SEARCH_RECENT_URL"] + "?" + urllib.parse.urlencode(
            {"query": query}
        )
        payload = fetch_json(search_url)
        tool_call("search_recent_posts", {"query": query}, payload)
        session_state["last_posts"] = payload.get("posts", [])
        tool_call(
            "write_session_state",
            {"key": "last_posts"},
            {"stored": True, "count": len(session_state["last_posts"])},
        )
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, render_post_lines(session_state["last_posts"]))
        return

    username = extract_username(text)
    if username:
        emit("model_turn_completed", stop_reason="tool_call")
        user_url = os.environ["HARNESS_X_USER_POSTS_URL"].rstrip("/") + "/" + username
        payload = fetch_json(user_url)
        tool_call("get_user_posts", {"username": username}, payload)
        session_state["last_posts"] = payload.get("posts", [])
        tool_call(
            "write_session_state",
            {"key": "last_posts"},
            {"stored": True, "count": len(session_state["last_posts"])},
        )
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, render_post_lines(session_state["last_posts"]))
        return

    if "first post" in lowered or "that search" in lowered:
        emit("model_turn_completed", stop_reason="tool_call")
        tool_call(
            "read_session_state",
            {"key": "last_posts"},
            {"value": session_state.get("last_posts", [])},
        )
        posts = session_state.get("last_posts", [])
        if posts:
            answer = [f"- The first post says: {posts[0].get('text', '')}"]
        else:
            answer = ["- No cached posts are available in this session."]
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, answer)
        return

    if extract_url(text):
        emit("model_turn_completed", stop_reason="tool_call")
        source_url = extract_url(text)
        payload = {"url": source_url}
        tool_call("fetch_url", payload, {"ok": True})
        source_text = fetch_text(source_url)
        session_state["last_fetch"] = {"url": source_url, "text": source_text}
        tool_call(
            "write_session_state",
            {"key": "last_fetch"},
            {"stored": True, "url": source_url},
        )
        emit("assistant_response_rendered")
        emit("loop_stopped", stop_reason="final_response")
        print_summary(prompt, render_post_lines([{"author": "web", "text": summarize_text(source_text, 2)}]))
        return

    emit("model_turn_completed", stop_reason="unsupported")
    emit("assistant_response_rendered")
    emit("loop_stopped", stop_reason="unsupported")
    print_reason(prompt, "unsupported goal")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--prompt", default="Goal >")
    args = parser.parse_args()

    emit("session_started")
    sys.stdout.write("MiniAgentOS fixture\n")
    print_prompt(args.prompt)

    session_state: dict[str, object] = {}
    turn_index = 0
    while True:
        line = sys.stdin.readline()
        if not line:
            return 0
        command = line.strip()
        if not command:
            print_prompt(args.prompt)
            continue
        if command in {
            "status plain",
            "status inline",
            "trace on",
            "trace off",
            "debug on",
            "debug off",
        }:
            print_prompt(args.prompt)
            continue
        if command in {"status status", "trace status", "debug status", "openai-status"}:
            sys.stdout.write("ok\n")
            sys.stdout.flush()
            print_prompt(args.prompt)
            continue
        if command.startswith("openai-key "):
            sys.stdout.write("openai key stored\n")
            sys.stdout.flush()
            print_prompt(args.prompt)
            continue
        if command == "openai-clear":
            sys.stdout.write("openai key cleared\n")
            sys.stdout.flush()
            print_prompt(args.prompt)
            continue

        try:
            payload = json.loads(command)
        except json.JSONDecodeError:
            payload = None

        if isinstance(payload, dict) and "goal_id" in payload:
            handle_legacy_task(payload)
            print_prompt(args.prompt)
            continue

        handle_text_turn(args.prompt, command, session_state, turn_index)
        turn_index += 1


if __name__ == "__main__":
    raise SystemExit(main())
