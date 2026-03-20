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
        choices=("all", "request", "response", "system", "input", "output"),
        default="all",
        help="Limit the rendered view to one part of the exchange. Default: all.",
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
        return section in {"request_meta", "system", "input", "raw_request", "budget", "request_diff"}
    if focus == "response":
        return section in {"response_meta", "output", "raw_response", "budget", "response_diff"}
    if focus == "system":
        return section in {"system", "raw_request", "request_meta", "budget", "request_diff"}
    if focus == "input":
        return section in {"input", "raw_request", "request_meta", "budget", "request_diff"}
    if focus == "output":
        return section in {"output", "raw_response", "response_meta", "budget", "response_diff"}
    return True


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
    full: bool,
    raw: bool,
    budget: bool,
    diff: bool,
    focus: str,
    color: bool,
    include_source: bool,
) -> str:
    request = as_dict(row.get("request"))
    response = as_dict(row.get("response"))
    request_diff, response_diff = request_response_diffs(previous_row, row)
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
                display_text(request.get("input")),
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
    full: bool,
    raw: bool,
    budget: bool,
    diff: bool,
    focus: str,
    color: bool = False,
) -> str:
    rendered_rows = [
        render_text_row(
            row,
            idx,
            previous_row,
            source,
            full,
            raw,
            budget,
            diff,
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
    full: bool,
    raw: bool,
    budget: bool,
    diff: bool,
    focus: str,
    color: bool,
    notice: str | None = None,
) -> str:
    body = render_text(entries, source, full, raw, budget, diff, focus, color=color)
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
    full: bool,
    raw: bool,
    budget: bool,
    diff: bool,
    focus: str,
) -> str:
    parts = [f"# LLM API Log\n\nSource: `{source}`\n"]
    for idx, row, previous_row in entries:
        request = as_dict(row.get("request"))
        response = as_dict(row.get("response"))
        request_diff, response_diff = request_response_diffs(previous_row, row)
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
                    display_text(request.get("input")),
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
            for key in ("system", "input", "budget", "raw_request", "request_diff", "response_meta", "output", "raw_response", "response_diff")
        ):
            parts.append("")
    return "\n".join(parts).rstrip() + "\n"


def main() -> int:
    args = parse_args()
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
                    args.full,
                    args.raw,
                    args.budget,
                    args.diff,
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
                        all_rows = load_rows(source)
                        entries = build_view_entries(all_rows, args.turn)
                        sys.stdout.write(
                            render_follow_view(
                                entries,
                                source,
                                args.full,
                                args.raw,
                                args.budget,
                                args.diff,
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
                current_entries = build_view_entries(current_all_rows, args.turn)
                current_snapshot = rows_snapshot(current_entries)
                if len(current_entries) < last_len:
                    sys.stdout.write(
                        render_follow_view(
                            current_entries,
                            source,
                            args.full,
                            args.raw,
                            args.budget,
                            args.diff,
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
                                args.full,
                                args.raw,
                                args.budget,
                                args.diff,
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
                            args.full,
                            args.raw,
                            args.budget,
                            args.diff,
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
        render_markdown(entries, source, args.full, args.raw, args.budget, args.diff, args.focus)
        if args.markdown
        else render_text(entries, source, args.full, args.raw, args.budget, args.diff, args.focus, color=use_color(args.color))
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
