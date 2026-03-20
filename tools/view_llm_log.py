#!/usr/bin/env python3

from __future__ import annotations

import argparse
import difflib
import json
import math
import time
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Render llm_api_log.jsonl into a readable text or Markdown view."
    )
    parser.add_argument(
        "--file",
        help="Path to a specific llm_api_log.jsonl file. If omitted, the latest available log is used.",
    )
    parser.add_argument(
        "--latest",
        action="store_true",
        help="Open the latest available llm_api_log.jsonl. This is the default when --file is omitted.",
    )
    parser.add_argument(
        "--turn",
        type=int,
        help="Show only one 1-based turn number.",
    )
    parser.add_argument(
        "--markdown",
        action="store_true",
        help="Render as Markdown instead of plain text.",
    )
    parser.add_argument(
        "--full",
        action="store_true",
        help="Include instructions and request/response metadata in addition to input/output.",
    )
    parser.add_argument(
        "--raw",
        action="store_true",
        help="Include raw request and raw response payloads for each turn.",
    )
    parser.add_argument(
        "--diff",
        action="store_true",
        help="Show request/response diffs against the previous turn.",
    )
    parser.add_argument(
        "--budget",
        action="store_true",
        help="Show character and rough token estimates for request/response sections.",
    )
    parser.add_argument(
        "--focus",
        choices=("all", "request", "response", "system", "input", "output", "context", "memory"),
        default="all",
        help="Limit the rendered view to one part of the exchange. Default: all.",
    )
    parser.add_argument(
        "--show-context-sections",
        action="store_true",
        help="Split request.input into prompt sections such as Working memory and Known sources.",
    )
    parser.add_argument(
        "--show-memory-events",
        action="store_true",
        help="Read sibling trace.jsonl and show memory events leading into each model request.",
    )
    parser.add_argument(
        "--show-compaction",
        action="store_true",
        help="Read sibling trace.jsonl and show memory compaction events leading into each model request.",
    )
    parser.add_argument(
        "--output",
        help="Write rendered output to a file instead of stdout.",
    )
    parser.add_argument(
        "--follow",
        action="store_true",
        help="Keep watching the selected log and print newly appended turns as they arrive.",
    )
    parser.add_argument(
        "--follow-latest",
        action="store_true",
        help="Like --follow, but also switch to a newer latest log file when the agent is restarted.",
    )
    parser.add_argument(
        "--poll-sec",
        type=float,
        default=0.5,
        help="Polling interval used by --follow. Default: 0.5 seconds.",
    )
    parser.add_argument(
        "--color",
        choices=("auto", "always", "never"),
        default="auto",
        help="Colorize plain-text output. Default: auto.",
    )
    return parser.parse_args()


def find_latest_log(repo_root: Path) -> Path:
    preferred = sorted(
        (repo_root / "output" / "agent-manual").glob("*/llm_api_log.jsonl"),
        key=lambda path: path.stat().st_mtime,
    )
    if preferred:
        return preferred[-1]

    fallback = sorted(
        repo_root.glob("output/**/llm_api_log.jsonl"),
        key=lambda path: path.stat().st_mtime,
    )
    if fallback:
        return fallback[-1]

    raise FileNotFoundError("no llm_api_log.jsonl files found under output/")


def load_rows(path: Path) -> list[dict]:
    rows: list[dict] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        rows.append(json.loads(line))
    return rows


def as_dict(value: Any) -> dict:
    return value if isinstance(value, dict) else {}


def display_text(value: Any, default: str = "") -> str:
    if value is None:
        return default
    if isinstance(value, str):
        return value
    return json.dumps(value, ensure_ascii=False)


def parse_json_text(text: Any) -> Any:
    if not isinstance(text, str):
        return None
    stripped = text.strip()
    if not stripped:
        return None
    try:
        return json.loads(stripped)
    except json.JSONDecodeError:
        return None


def render_output_summary(response: dict) -> str:
    raw_text = response.get("text")
    parsed = parse_json_text(raw_text)
    if isinstance(parsed, dict):
        kind = parsed.get("type")
        if kind == "final":
            return display_text(parsed.get("response"), "(missing)")
        if kind == "tool":
            tool = display_text(parsed.get("tool"), "").strip()
            arguments = {
                key: value
                for key, value in parsed.items()
                if key not in ("type", "tool")
            }
            if not tool:
                return json.dumps(parsed, ensure_ascii=False)
            if not arguments:
                return f"[tool] {tool}"
            return f"[tool] {tool} {json.dumps(arguments, ensure_ascii=False)}"
        if kind == "refusal":
            return display_text(parsed.get("reason"), "(refusal)")
    return display_text(raw_text, "(missing)")


def estimate_tokens(text: str) -> int:
    if not text:
        return 0
    return max(1, math.ceil(len(text) / 4))


def raw_request_payload(row: dict) -> dict:
    request = as_dict(row.get("request"))
    return {
        "model": row.get("model"),
        "instructions": request.get("instructions"),
        "input": request.get("input"),
        "reasoning_effort": request.get("reasoning_effort"),
        "max_output_tokens": request.get("max_output_tokens"),
    }


def raw_response_payload(row: dict) -> dict:
    response = as_dict(row.get("response"))
    return {
        "http_status": response.get("http_status"),
        "body_truncated": response.get("body_truncated"),
        "parsed_output": response.get("parsed_output"),
        "text": response.get("text"),
    }


def pretty_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True)


def unified_text_diff(previous: str, current: str) -> str:
    diff_lines = list(
        difflib.unified_diff(
            previous.splitlines(),
            current.splitlines(),
            fromfile="previous",
            tofile="current",
            lineterm="",
        )
    )
    if not diff_lines:
        return "(no change)"
    return "\n".join(diff_lines)


def request_response_diffs(previous_row: dict | None, row: dict) -> tuple[str, str]:
    if previous_row is None:
        return "(first turn)", "(first turn)"
    return (
        unified_text_diff(pretty_json(raw_request_payload(previous_row)), pretty_json(raw_request_payload(row))),
        unified_text_diff(pretty_json(raw_response_payload(previous_row)), pretty_json(raw_response_payload(row))),
    )


def budget_summary(row: dict) -> str:
    request = as_dict(row.get("request"))
    response = as_dict(row.get("response"))
    instructions = display_text(request.get("instructions"))
    input_text = display_text(request.get("input"))
    output_text = display_text(response.get("text"))
    total_request = instructions + input_text
    return "\n".join(
        [
            f"request.instructions: chars={len(instructions)} est_tokens~={estimate_tokens(instructions)}",
            f"request.input: chars={len(input_text)} est_tokens~={estimate_tokens(input_text)}",
            f"request.total: chars={len(total_request)} est_tokens~={estimate_tokens(total_request)}",
            f"response.text: chars={len(output_text)} est_tokens~={estimate_tokens(output_text)}",
        ]
    )


def build_view_entries(rows: list[dict], turn: int | None) -> list[tuple[int, dict, dict | None]]:
    if turn is not None:
        if turn < 1 or turn > len(rows):
            raise IndexError(f"turn {turn} is out of range for {len(rows)} rows")
        previous = rows[turn - 2] if turn > 1 else None
        return [(turn, rows[turn - 1], previous)]

    entries: list[tuple[int, dict, dict | None]] = []
    previous: dict | None = None
    for idx, row in enumerate(rows, 1):
        entries.append((idx, row, previous))
        previous = row
    return entries


def focus_allows(focus: str, section: str) -> bool:
    if focus == "all":
        return True
    if focus == "request":
        return section in {"request_meta", "system", "input", "context_sections", "trace_budget", "raw_request", "budget", "request_diff"}
    if focus == "response":
        return section in {"response_meta", "output", "raw_response", "budget", "response_diff"}
    if focus == "system":
        return section in {"system", "raw_request", "request_meta", "budget", "request_diff"}
    if focus == "input":
        return section in {"input", "context_sections", "trace_budget", "raw_request", "request_meta", "budget", "request_diff"}
    if focus == "output":
        return section in {"output", "raw_response", "response_meta", "budget", "response_diff"}
    if focus == "context":
        return section in {"system", "input", "context_sections", "trace_budget", "raw_request", "request_meta", "budget", "request_diff"}
    if focus == "memory":
        return section in {"context_sections", "trace_budget", "memory_events", "compaction"}
    return True


def find_trace_for_log(source: Path) -> Path | None:
    candidate = source.with_name("trace.jsonl")
    return candidate if candidate.exists() else None


def load_trace_events(path: Path | None) -> list[dict]:
    if path is None:
        return []
    events: list[dict] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(obj, dict):
            events.append(obj)
    return events


TOP_LEVEL_SECTION_NAMES = {
    "Current request",
    "Latest tool result",
    "Session state",
    "Recent conversation",
    "Working memory",
    "Known sources",
    "Workspace memory",
    "Timely research requirement",
    "Research completion requirement",
    "Memory inspection requirement",
    "Execution requirement",
}


def looks_like_section_header(line: str) -> bool:
    stripped = line.strip()
    if not stripped.endswith(":"):
        return False
    name = stripped[:-1].strip()
    if not name or len(name) > 80:
        return False
    return name in TOP_LEVEL_SECTION_NAMES


def parse_input_sections(text: str) -> list[tuple[str, str]]:
    if not text:
        return []
    sections: list[tuple[str, str]] = []
    current_name: str | None = None
    current_lines: list[str] = []

    for raw_line in text.splitlines():
        if looks_like_section_header(raw_line):
            if current_name is not None:
                sections.append((current_name, "\n".join(current_lines).strip("\n")))
            current_name = raw_line.strip()[:-1].strip()
            current_lines = []
            continue
        if current_name is not None:
            current_lines.append(raw_line)

    if current_name is not None:
        sections.append((current_name, "\n".join(current_lines).strip("\n")))
    return sections


def trace_context_section_map(trace_events: list[dict], interaction_id: int) -> dict[str, int]:
    result: dict[str, int] = {}
    for event in trace_events:
        if event.get("event") != "context_section_snapshot":
            continue
        if int(event.get("interaction_id", 0) or 0) != interaction_id:
            continue
        name = display_text(event.get("name")).strip()
        if not name:
            continue
        result[name] = int(event.get("chars", 0) or 0)
    return result


def trace_budget_for_row(trace_events: list[dict], row: dict, previous_row: dict | None) -> dict | None:
    request_ts = int(row.get("ts_ms_request", 0) or 0)
    if request_ts <= 0:
        return None
    previous_ts = 0
    if previous_row is not None:
        previous_ts = int(previous_row.get("ts_ms_response", 0) or previous_row.get("ts_ms_request", 0) or 0)
    step = int(row.get("step", 0) or 0)
    match: dict | None = None
    for event in trace_events:
        if event.get("event") != "context_budget_snapshot":
            continue
        ts = int(event.get("ts_ms", 0) or 0)
        if ts <= previous_ts or ts > request_ts:
            continue
        if int(event.get("step", 0) or 0) != step:
            continue
        match = event
    return match


def trace_memory_window(trace_events: list[dict], row: dict, previous_row: dict | None) -> list[dict]:
    request_ts = int(row.get("ts_ms_request", 0) or 0)
    if request_ts <= 0:
        return []
    previous_ts = 0
    if previous_row is not None:
        previous_ts = int(previous_row.get("ts_ms_response", 0) or previous_row.get("ts_ms_request", 0) or 0)
    interesting = {"memory_event", "memory_entry_snapshot", "memory_compacted"}
    return [
        event
        for event in trace_events
        if event.get("event") in interesting
        and previous_ts < int(event.get("ts_ms", 0) or 0) <= request_ts
    ]


def format_context_sections(input_text: str, trace_events: list[dict], row: dict) -> str:
    sections = parse_input_sections(input_text)
    if not sections:
        return "(no parsed sections)"
    section_chars = trace_context_section_map(trace_events, int(row.get("interaction_id", 0) or 0))
    rendered: list[str] = []
    for name, body in sections:
        chars_suffix = ""
        if name in section_chars:
            chars_suffix = f"  [chars={section_chars[name]}]"
        rendered.append(f"## {name}{chars_suffix}")
        rendered.append(body if body else "(empty)")
        rendered.append("")
    return "\n".join(rendered).rstrip()


def format_trace_budget(event: dict | None) -> str:
    if not event:
        return "(trace budget unavailable)"
    fields = [
        "instructions_chars",
        "current_request_chars",
        "latest_tool_result_chars",
        "working_memory_chars",
        "known_sources_chars",
        "workspace_memory_chars",
        "session_state_chars",
        "recent_conversation_chars",
        "estimated_total_tokens",
    ]
    return "\n".join(f"{name}: {int(event.get(name, 0) or 0)}" for name in fields)


def format_memory_events(events: list[dict]) -> str:
    if not events:
        return "(no memory events in this request window)"
    lines: list[str] = []
    for event in events:
        name = display_text(event.get("event"))
        if name == "memory_event":
            lines.append(
                f"- {display_text(event.get('entry_id'))} [{display_text(event.get('kind'))}] "
                f"{display_text(event.get('from_state'), '(new)')} -> {display_text(event.get('to_state'))} "
                f"(turn={display_text(event.get('turn_index'))})"
            )
        elif name == "memory_entry_snapshot":
            summary = display_text(event.get("summary"))
            lines.append(
                f"- snapshot {display_text(event.get('id'))} [{display_text(event.get('kind'))}/{display_text(event.get('state'))}] "
                f"chars={display_text(event.get('chars'))} summary={summary}"
            )
        elif name == "memory_compacted":
            lines.append(
                f"- compacted {display_text(event.get('entry_id'))} [{display_text(event.get('kind'))}] "
                f"{display_text(event.get('from_state'), '(n/a)')} -> {display_text(event.get('to_state'))} "
                f"retained={display_text(event.get('retained_chars'))} dropped={display_text(event.get('dropped_chars'))} "
                f"mode={display_text(event.get('mode'))}"
            )
    return "\n".join(lines)


def format_compaction_events(events: list[dict]) -> str:
    compactions = [event for event in events if event.get("event") == "memory_compacted"]
    if not compactions:
        return "(no compaction events in this request window)"
    return "\n".join(
        f"- {display_text(event.get('entry_id'))} [{display_text(event.get('kind'))}] "
        f"retained={display_text(event.get('retained_chars'))} dropped={display_text(event.get('dropped_chars'))} "
        f"mode={display_text(event.get('mode'))}"
        for event in compactions
    )


class Colors:
    RESET = "\033[0m"
    BOLD = "\033[1m"
    DIM = "\033[2m"
    CYAN = "\033[36m"
    GREEN = "\033[32m"
    YELLOW = "\033[33m"
    MAGENTA = "\033[35m"
    BLUE = "\033[34m"


def use_color(mode: str) -> bool:
    if mode == "always":
        return True
    if mode == "never":
        return False
    return sys.stdout.isatty()


def paint(text: str, *styles: str, enabled: bool) -> str:
    if not enabled or not styles:
        return text
    return "".join(styles) + text + Colors.RESET


def render_text_row(
    row: dict,
    idx: int,
    previous_row: dict | None,
    source: Path,
    trace_events: list[dict],
    full: bool,
    raw: bool,
    budget: bool,
    diff: bool,
    show_context_sections: bool,
    show_memory_events: bool,
    show_compaction: bool,
    focus: str,
    color: bool,
    include_source: bool,
) -> str:
    request = as_dict(row.get("request"))
    response = as_dict(row.get("response"))
    request_diff, response_diff = request_response_diffs(previous_row, row)
    input_text = display_text(request.get("input"))
    trace_budget = trace_budget_for_row(trace_events, row, previous_row)
    memory_events = trace_memory_window(trace_events, row, previous_row)
    parts: list[str] = []
    if include_source:
        parts.append(paint(f"Source: {source}", Colors.DIM, enabled=color))
    parts.extend(
        [
            "",
            paint("=" * 100, Colors.CYAN, enabled=color),
            paint(
                f"TURN {idx} | phase={row.get('phase', '')} | model={row.get('model', '')}",
                Colors.BOLD,
                Colors.CYAN,
                enabled=color,
            ),
            paint("=" * 100, Colors.CYAN, enabled=color),
        ]
    )
    if full:
        if focus_allows(focus, "request_meta"):
            parts.extend(
                [
                    "",
                    paint("[REQUEST META]", Colors.MAGENTA, Colors.BOLD, enabled=color),
                    f"interaction_id: {row.get('interaction_id', '')}",
                    f"step: {row.get('step', '')}",
                    f"ts_ms_request: {row.get('ts_ms_request', '')}",
                    f"reasoning_effort: {request.get('reasoning_effort', '')}",
                    f"max_output_tokens: {request.get('max_output_tokens', '')}",
                ]
            )
    if focus_allows(focus, "system"):
        parts.extend(
            [
                "",
                paint("[SYSTEM / TOOLS]", Colors.BLUE, Colors.BOLD, enabled=color),
                display_text(request.get("instructions")),
            ]
        )
    if focus_allows(focus, "input"):
        parts.extend(
            [
                "",
                paint("[INPUT]", Colors.GREEN, Colors.BOLD, enabled=color),
                input_text,
            ]
        )
    if show_context_sections and focus_allows(focus, "context_sections"):
        parts.extend(
            [
                "",
                paint("[CONTEXT SECTIONS]", Colors.GREEN, Colors.BOLD, enabled=color),
                format_context_sections(input_text, trace_events, row),
            ]
        )
    if (show_context_sections or focus == "memory" or focus == "context") and focus_allows(focus, "trace_budget"):
        parts.extend(
            [
                "",
                paint("[TRACE CONTEXT BUDGET]", Colors.MAGENTA, Colors.BOLD, enabled=color),
                format_trace_budget(trace_budget),
            ]
        )
    if budget and focus_allows(focus, "budget"):
        parts.extend(
            [
                "",
                paint("[BUDGET]", Colors.MAGENTA, Colors.BOLD, enabled=color),
                budget_summary(row),
            ]
        )
    if raw and focus_allows(focus, "raw_request"):
        parts.extend(
            [
                "",
                paint("[RAW REQUEST]", Colors.BLUE, Colors.BOLD, enabled=color),
                pretty_json(raw_request_payload(row)),
            ]
        )
    if diff and focus_allows(focus, "request_diff"):
        parts.extend(
            [
                "",
                paint("[REQUEST DIFF]", Colors.BLUE, Colors.BOLD, enabled=color),
                request_diff,
            ]
        )
    if show_memory_events and focus_allows(focus, "memory_events"):
        parts.extend(
            [
                "",
                paint("[MEMORY EVENTS]", Colors.BLUE, Colors.BOLD, enabled=color),
                format_memory_events(memory_events),
            ]
        )
    if show_compaction and focus_allows(focus, "compaction"):
        parts.extend(
            [
                "",
                paint("[COMPACTION]", Colors.BLUE, Colors.BOLD, enabled=color),
                format_compaction_events(memory_events),
            ]
        )
    if full:
        if focus_allows(focus, "response_meta"):
            parts.extend(
                [
                    "",
                    paint("[RESPONSE META]", Colors.MAGENTA, Colors.BOLD, enabled=color),
                    f"ts_ms_response: {row.get('ts_ms_response', '')}",
                    f"http_status: {response.get('http_status', '')}",
                    f"parsed_output: {response.get('parsed_output', '')}",
                    f"body_truncated: {response.get('body_truncated', '')}",
                ]
            )
    if focus_allows(focus, "output"):
        parts.extend(
            [
                "",
                paint("[OUTPUT]", Colors.YELLOW, Colors.BOLD, enabled=color),
                render_output_summary(response),
            ]
        )
    if raw and focus_allows(focus, "raw_response"):
        parts.extend(
            [
                "",
                paint("[RAW RESPONSE]", Colors.YELLOW, Colors.BOLD, enabled=color),
                pretty_json(raw_response_payload(row)),
            ]
        )
    elif full and focus_allows(focus, "raw_response"):
        parts.extend(
            [
                "",
                paint("[RAW RESPONSE]", Colors.YELLOW, Colors.BOLD, enabled=color),
                display_text(response.get("text"), "(missing)"),
            ]
        )
    if diff and focus_allows(focus, "response_diff"):
        parts.extend(
            [
                "",
                paint("[RESPONSE DIFF]", Colors.YELLOW, Colors.BOLD, enabled=color),
                response_diff,
            ]
        )
    return "\n".join(parts).rstrip() + "\n"


def render_text(
    entries: list[tuple[int, dict, dict | None]],
    source: Path,
    trace_events: list[dict],
    full: bool,
    raw: bool,
    budget: bool,
    diff: bool,
    show_context_sections: bool,
    show_memory_events: bool,
    show_compaction: bool,
    focus: str,
    color: bool = False,
) -> str:
    rendered_rows = [
        render_text_row(
            row,
            idx,
            previous_row,
            source,
            trace_events,
            full,
            raw,
            budget,
            diff,
            show_context_sections,
            show_memory_events,
            show_compaction,
            focus,
            color=color,
            include_source=(display_idx == 0),
        )
        for display_idx, (idx, row, previous_row) in enumerate(entries)
    ]
    return "\n".join(chunk.rstrip("\n") for chunk in rendered_rows).rstrip() + "\n"


def rows_snapshot(entries: list[tuple[int, dict, dict | None]]) -> str:
    return json.dumps(
        [{"idx": idx, "row": row} for idx, row, _ in entries],
        ensure_ascii=False,
        sort_keys=True,
    )


def render_follow_view(
    entries: list[tuple[int, dict, dict | None]],
    source: Path,
    trace_events: list[dict],
    full: bool,
    raw: bool,
    budget: bool,
    diff: bool,
    show_context_sections: bool,
    show_memory_events: bool,
    show_compaction: bool,
    focus: str,
    color: bool,
    notice: str | None = None,
) -> str:
    body = render_text(
        entries,
        source,
        trace_events,
        full,
        raw,
        budget,
        diff,
        show_context_sections,
        show_memory_events,
        show_compaction,
        focus,
        color=color,
    )
    if notice:
        notice_text = paint(notice, Colors.MAGENTA, Colors.BOLD, enabled=color) + "\n"
    else:
        notice_text = ""
    if sys.stdout.isatty():
        return "\033[2J\033[H" + notice_text + body
    return ("\n" + notice_text if notice_text else "") + body


def render_markdown(
    entries: list[tuple[int, dict, dict | None]],
    source: Path,
    trace_events: list[dict],
    full: bool,
    raw: bool,
    budget: bool,
    diff: bool,
    show_context_sections: bool,
    show_memory_events: bool,
    show_compaction: bool,
    focus: str,
) -> str:
    parts = [f"# LLM API Log\n\nSource: `{source}`\n"]
    for idx, row, previous_row in entries:
        request = as_dict(row.get("request"))
        response = as_dict(row.get("response"))
        request_diff, response_diff = request_response_diffs(previous_row, row)
        input_text = display_text(request.get("input"))
        trace_budget = trace_budget_for_row(trace_events, row, previous_row)
        memory_events = trace_memory_window(trace_events, row, previous_row)
        parts.extend(
            [
                f"## Turn {idx}",
                f"- phase: `{row.get('phase', '')}`",
                f"- model: `{row.get('model', '')}`",
            ]
        )
        if full and focus_allows(focus, "request_meta"):
            parts.extend(
                [
                    f"- interaction_id: `{row.get('interaction_id', '')}`",
                    f"- step: `{row.get('step', '')}`",
                    f"- ts_ms_request: `{row.get('ts_ms_request', '')}`",
                    f"- reasoning_effort: `{request.get('reasoning_effort', '')}`",
                    f"- max_output_tokens: `{request.get('max_output_tokens', '')}`",
                ]
            )
        if focus_allows(focus, "system"):
            parts.extend(
                [
                    "",
                    "### System / Tools",
                    "```text",
                    display_text(request.get("instructions")),
                    "```",
                ]
            )
        if focus_allows(focus, "input"):
            parts.extend(
                [
                    "",
                    "### Input",
                    "```text",
                    input_text,
                    "```",
                ]
            )
        if show_context_sections and focus_allows(focus, "context_sections"):
            parts.extend(
                [
                    "",
                    "### Context Sections",
                    "```text",
                    format_context_sections(input_text, trace_events, row),
                    "```",
                ]
            )
        if (show_context_sections or focus == "memory" or focus == "context") and focus_allows(focus, "trace_budget"):
            parts.extend(
                [
                    "",
                    "### Trace Context Budget",
                    "```text",
                    format_trace_budget(trace_budget),
                    "```",
                ]
            )
        if budget and focus_allows(focus, "budget"):
            parts.extend(
                [
                    "",
                    "### Budget",
                    "```text",
                    budget_summary(row),
                    "```",
                ]
            )
        if raw and focus_allows(focus, "raw_request"):
            parts.extend(
                [
                    "",
                    "### Raw Request",
                    "```json",
                    pretty_json(raw_request_payload(row)),
                    "```",
                ]
            )
        if diff and focus_allows(focus, "request_diff"):
            parts.extend(
                [
                    "",
                    "### Request Diff",
                    "```diff",
                    request_diff,
                    "```",
                ]
            )
        if show_memory_events and focus_allows(focus, "memory_events"):
            parts.extend(
                [
                    "",
                    "### Memory Events",
                    "```text",
                    format_memory_events(memory_events),
                    "```",
                ]
            )
        if show_compaction and focus_allows(focus, "compaction"):
            parts.extend(
                [
                    "",
                    "### Compaction",
                    "```text",
                    format_compaction_events(memory_events),
                    "```",
                ]
            )
        if full and focus_allows(focus, "response_meta"):
            parts.extend(
                [
                    "",
                    "### Response Meta",
                    f"- ts_ms_response: `{row.get('ts_ms_response', '')}`",
                    f"- http_status: `{response.get('http_status', '')}`",
                    f"- parsed_output: `{response.get('parsed_output', '')}`",
                    f"- body_truncated: `{response.get('body_truncated', '')}`",
                ]
            )
        if focus_allows(focus, "output"):
            parts.extend(
                [
                    "",
                    "### Output",
                    "```text",
                    render_output_summary(response),
                    "```",
                ]
            )
        if raw and focus_allows(focus, "raw_response"):
            parts.extend(
                [
                    "",
                    "### Raw Response",
                    "```json",
                    pretty_json(raw_response_payload(row)),
                    "```",
                ]
            )
        elif full and focus_allows(focus, "raw_response"):
            parts.extend(
                [
                    "",
                    "### Raw Response",
                    "```text",
                    display_text(response.get("text"), "(missing)"),
                    "```",
                ]
            )
        if diff and focus_allows(focus, "response_diff"):
            parts.extend(
                [
                    "",
                    "### Response Diff",
                    "```diff",
                    response_diff,
                    "```",
                ]
            )
        if any(
            focus_allows(focus, key)
            for key in ("system", "input", "context_sections", "trace_budget", "memory_events", "compaction", "budget", "raw_request", "request_diff", "response_meta", "output", "raw_response", "response_diff")
        ):
            parts.append("")
    return "\n".join(parts).rstrip() + "\n"


def main() -> int:
    args = parse_args()
    if args.focus == "context":
        args.show_context_sections = True
    elif args.focus == "memory":
        args.show_context_sections = True
        args.show_memory_events = True
        args.show_compaction = True
    if (args.follow or args.follow_latest) and args.output:
        print("--follow/--follow-latest cannot be used with --output", file=sys.stderr)
        return 2
    if (args.follow or args.follow_latest) and args.markdown:
        print("--follow/--follow-latest cannot be used with --markdown", file=sys.stderr)
        return 2
    if args.follow_latest and args.file:
        print("--follow-latest cannot be used with --file", file=sys.stderr)
        return 2
    try:
        source = Path(args.file).expanduser().resolve() if args.file else find_latest_log(REPO_ROOT)
        trace_source = find_trace_for_log(source)
        trace_events = load_trace_events(trace_source)
        all_rows = load_rows(source)
        entries = build_view_entries(all_rows, args.turn)
    except FileNotFoundError as exc:
        print(str(exc), file=sys.stderr)
        return 2
    except IndexError as exc:
        print(str(exc), file=sys.stderr)
        return 2
    except json.JSONDecodeError as exc:
        print(f"failed to parse JSONL: {exc}", file=sys.stderr)
        return 2

    if args.follow or args.follow_latest:
        color = use_color(args.color)
        last_len = 0
        last_snapshot = ""
        previous_entries: list[tuple[int, dict, dict | None]] = []
        if entries:
            sys.stdout.write(
                render_follow_view(
                    entries,
                    source,
                    trace_events,
                    args.full,
                    args.raw,
                    args.budget,
                    args.diff,
                    args.show_context_sections,
                    args.show_memory_events,
                    args.show_compaction,
                    args.focus,
                    color=color,
                )
            )
            sys.stdout.flush()
            last_len = len(entries)
            last_snapshot = rows_snapshot(entries)
            previous_entries = entries
        try:
            while True:
                time.sleep(args.poll_sec)
                if args.follow_latest:
                    latest_source = find_latest_log(REPO_ROOT)
                    if latest_source != source:
                        source = latest_source
                        trace_source = find_trace_for_log(source)
                        trace_events = load_trace_events(trace_source)
                        all_rows = load_rows(source)
                        entries = build_view_entries(all_rows, args.turn)
                        sys.stdout.write(
                            render_follow_view(
                                entries,
                                source,
                                trace_events,
                                args.full,
                                args.raw,
                                args.budget,
                                args.diff,
                                args.show_context_sections,
                                args.show_memory_events,
                                args.show_compaction,
                                args.focus,
                                color=color,
                                notice=f"[viewer] switched to newer log: {source}",
                            )
                        )
                        sys.stdout.flush()
                        last_len = len(entries)
                        last_snapshot = rows_snapshot(entries)
                        previous_entries = entries
                        continue
                current_all_rows = load_rows(source)
                trace_events = load_trace_events(trace_source)
                current_entries = build_view_entries(current_all_rows, args.turn)
                current_snapshot = rows_snapshot(current_entries)
                if len(current_entries) < last_len:
                    sys.stdout.write(
                        render_follow_view(
                            current_entries,
                            source,
                            trace_events,
                            args.full,
                            args.raw,
                            args.budget,
                            args.diff,
                            args.show_context_sections,
                            args.show_memory_events,
                            args.show_compaction,
                            args.focus,
                            color=color,
                            notice=f"[viewer] log was truncated or restarted: {source}",
                        )
                    )
                    sys.stdout.flush()
                    last_len = len(current_entries)
                    last_snapshot = current_snapshot
                    previous_entries = current_entries
                    continue
                if current_snapshot == last_snapshot:
                    continue

                current_rows_only = [row for _, row, _ in current_entries]
                previous_rows_only = [row for _, row, _ in previous_entries]
                if len(current_entries) > last_len and current_rows_only[:last_len] == previous_rows_only:
                    for display_idx in range(last_len, len(current_entries)):
                        idx, row, previous_row = current_entries[display_idx]
                        sys.stdout.write(
                            render_text_row(
                                row,
                                idx,
                                previous_row,
                                source,
                                trace_events,
                                args.full,
                                args.raw,
                                args.budget,
                                args.diff,
                                args.show_context_sections,
                                args.show_memory_events,
                                args.show_compaction,
                                args.focus,
                                color=color,
                                include_source=(last_len == 0 and display_idx == 0),
                            )
                        )
                    sys.stdout.flush()
                else:
                    sys.stdout.write(
                        render_follow_view(
                            current_entries,
                            source,
                            trace_events,
                            args.full,
                            args.raw,
                            args.budget,
                            args.diff,
                            args.show_context_sections,
                            args.show_memory_events,
                            args.show_compaction,
                            args.focus,
                            color=color,
                        )
                    )
                    sys.stdout.flush()
                last_len = len(current_entries)
                last_snapshot = current_snapshot
                previous_entries = current_entries
        except KeyboardInterrupt:
            return 0

    rendered = (
        render_markdown(
            entries,
            source,
            trace_events,
            args.full,
            args.raw,
            args.budget,
            args.diff,
            args.show_context_sections,
            args.show_memory_events,
            args.show_compaction,
            args.focus,
        )
        if args.markdown
        else render_text(
            entries,
            source,
            trace_events,
            args.full,
            args.raw,
            args.budget,
            args.diff,
            args.show_context_sections,
            args.show_memory_events,
            args.show_compaction,
            args.focus,
            color=use_color(args.color),
        )
    )

    if args.output:
        output_path = Path(args.output).expanduser().resolve()
        output_path.write_text(rendered, encoding="utf-8")
        print(output_path)
        return 0

    sys.stdout.write(rendered)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
