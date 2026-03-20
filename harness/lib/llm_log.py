from __future__ import annotations

import json
from pathlib import Path


def extract_llm_api_log(trace_events: list[dict]) -> list[dict]:
    pending: dict[int, dict] = {}
    rows: list[dict] = []

    for event in trace_events:
        if not isinstance(event, dict):
            continue
        event_name = str(event.get("event", ""))
        if event_name == "model_request_snapshot":
            interaction_id = int(event.get("interaction_id", 0) or 0)
            if interaction_id == 0:
                interaction_id = len(rows) + len(pending) + 1
            pending[interaction_id] = {
                "interaction_id": interaction_id,
                "step": int(event.get("step", 0) or 0),
                "phase": event.get("phase"),
                "ts_ms_request": event.get("ts_ms"),
                "model": event.get("model"),
                "request": {
                    "instructions": event.get("instructions"),
                    "input": event.get("input"),
                    "reasoning_effort": event.get("reasoning_effort"),
                    "max_output_tokens": event.get("max_output_tokens"),
                },
            }
            continue

        if event_name != "model_response_snapshot":
            continue

        interaction_id = int(event.get("interaction_id", 0) or 0)
        row = pending.pop(interaction_id, None)
        if row is None:
            row = {
                "interaction_id": interaction_id,
                "step": int(event.get("step", 0) or 0),
                "phase": event.get("phase"),
                "ts_ms_request": None,
                "model": event.get("model"),
                "request": None,
            }
        row["ts_ms_response"] = event.get("ts_ms")
        row["response"] = {
            "http_status": event.get("http_status"),
            "body_truncated": event.get("body_truncated"),
            "parsed_output": event.get("parsed_output"),
            "text": event.get("text"),
        }
        rows.append(row)

    for interaction_id in sorted(pending):
        rows.append(pending[interaction_id])

    return rows


def write_llm_api_log_jsonl(path: Path, rows: list[dict]) -> None:
    lines = [json.dumps(row, ensure_ascii=False) for row in rows]
    path.write_text("".join(line + "\n" for line in lines), encoding="utf-8")
