from __future__ import annotations

import atexit
import copy
import json
from pathlib import Path


def _estimate_tokens(text: str) -> int:
    if not text:
        return 0
    return max(1, (len(text) + 3) // 4)


def _join_nonempty(parts: list[str]) -> str:
    return "\n".join(part for part in parts if part)


class M7Substrate:
    def __init__(self, output_dir: Path | None) -> None:
        self.output_dir = output_dir.resolve() if output_dir is not None else None
        self.entries: dict[str, dict] = {}
        self.entry_order: list[str] = []
        self.events: list[dict] = []
        self.context_turns: list[dict] = []
        self.budget_turns: list[dict] = []
        self.checkpoints: list[dict] = []
        self._checkpoint_payloads: dict[str, list[dict]] = {}
        self._next_memory_id = 1
        self._next_checkpoint_id = 1
        atexit.register(self.flush_artifacts)

    def available(self) -> bool:
        return self.output_dir is not None

    def _write_artifact(self, name: str, payload) -> None:
        if self.output_dir is None:
            return
        (self.output_dir / name).write_text(
            json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )

    def flush_artifacts(self) -> None:
        self._write_artifact(
            "memory_snapshot.json",
            {"entries": [copy.deepcopy(self.entries[entry_id]) for entry_id in self.entry_order]},
        )
        self._write_artifact("memory_events.json", {"events": self.events})
        self._write_artifact("context_snapshot.json", {"turns": self.context_turns})
        self._write_artifact("context_budget.json", {"turns": self.budget_turns})
        if self.checkpoints:
            self._write_artifact("checkpoint_snapshot.json", {"checkpoints": self.checkpoints})

    def _alloc_memory_id(self) -> str:
        value = f"mem-{self._next_memory_id}"
        self._next_memory_id += 1
        return value

    def _alloc_checkpoint_id(self) -> str:
        value = f"ckpt-{self._next_checkpoint_id}"
        self._next_checkpoint_id += 1
        return value

    def _error(self, code: str, message: str, **extra) -> dict:
        payload = {
            "ok": False,
            "error": {
                "code": code,
                "message": message,
            },
        }
        payload.update(extra)
        return payload

    def add_memory(
        self,
        kind: str,
        summary: str,
        *,
        source: str,
        state: str = "derived",
        detail: str = "",
        turn_index: int = 0,
        entry_id: str | None = None,
    ) -> dict:
        if kind not in {"task", "source", "workspace", "execution", "conversation"}:
            return self._error("invalid_kind", "unknown memory kind", kind=kind)
        entry_id = entry_id or self._alloc_memory_id()
        existed = entry_id in self.entries
        entry = {
            "id": entry_id,
            "kind": kind,
            "summary": summary,
            "detail": detail,
            "source": source,
            "state": state,
            "created_turn": self.entries.get(entry_id, {}).get("created_turn", turn_index),
            "updated_turn": turn_index,
            "chars": len((summary or "") + (detail or "")),
            "estimated_tokens": _estimate_tokens((summary or "") + "\n" + (detail or "")),
        }
        self.entries[entry_id] = entry
        if not existed:
            self.entry_order.append(entry_id)
            self.events.append(
                {
                    "turn_index": turn_index,
                    "event": "memory_added",
                    "entry_id": entry_id,
                    "kind": kind,
                }
            )
        else:
            self.events.append(
                {
                    "turn_index": turn_index,
                    "event": "memory_updated",
                    "entry_id": entry_id,
                    "kind": kind,
                }
            )
        self.flush_artifacts()
        return {"ok": True, "entry": copy.deepcopy(entry)}

    def list_memory(self, kind: str | None = None, limit: int = 20) -> dict:
        if limit < 1 or limit > 100:
            return self._error("policy_denied", "limit exceeds policy bound", limit=limit)
        entries = []
        for entry_id in self.entry_order:
            entry = self.entries[entry_id]
            if kind and entry.get("kind") != kind:
                continue
            entries.append(
                {
                    "id": entry["id"],
                    "kind": entry["kind"],
                    "summary": entry["summary"],
                    "source": entry["source"],
                    "state": entry["state"],
                }
            )
            if len(entries) >= limit:
                break
        return {
            "ok": True,
            "entries": entries,
            "truncated": len(entries) < sum(
                1 for entry_id in self.entry_order if not kind or self.entries[entry_id].get("kind") == kind
            ),
        }

    def read_memory(self, entry_id: str) -> dict:
        entry = self.entries.get(entry_id)
        if entry is None:
            return self._error("unknown_memory_id", "memory entry was not found", id=entry_id)
        return {"ok": True, "entry": copy.deepcopy(entry)}

    def compact_memory(self, ids: list[str], mode: str = "bounded_summary", turn_index: int = 0) -> dict:
        updated: list[dict] = []
        for entry_id in ids:
            entry = self.entries.get(entry_id)
            if entry is None:
                return self._error("unknown_memory_id", "memory entry was not found", id=entry_id)
            before = entry.get("state", "raw")
            summary = str(entry.get("summary", ""))
            detail = str(entry.get("detail", ""))
            compacted_summary = summary if len(summary) <= 120 else summary[:117].rstrip() + "..."
            compacted_detail = detail if len(detail) <= 240 else detail[:237].rstrip() + "..."
            entry["summary"] = compacted_summary
            entry["detail"] = compacted_detail
            entry["state"] = "compacted"
            entry["updated_turn"] = turn_index
            entry["chars"] = len(compacted_summary + compacted_detail)
            entry["estimated_tokens"] = _estimate_tokens(compacted_summary + "\n" + compacted_detail)
            self.events.append(
                {
                    "turn_index": turn_index,
                    "event": "memory_compacted",
                    "entry_id": entry_id,
                    "kind": entry.get("kind"),
                    "from_state": before,
                    "to_state": "compacted",
                    "mode": mode,
                }
            )
            updated.append({"id": entry_id, "state": "compacted"})
        self.flush_artifacts()
        return {"ok": True, "updated": updated}

    def _section_payload(self, kind: str) -> tuple[str, list[str]]:
        entry_ids: list[str] = []
        lines: list[str] = []
        for entry_id in self.entry_order:
            entry = self.entries[entry_id]
            entry_kind = str(entry.get("kind", ""))
            if kind == "working" and entry_kind not in {"task", "execution", "conversation"}:
                continue
            if kind == "source" and entry_kind != "source":
                continue
            if kind == "workspace" and entry_kind != "workspace":
                continue
            entry_ids.append(entry_id)
            lines.append(str(entry.get("summary", "")))
        return _join_nonempty(lines), entry_ids

    def record_context_snapshot(
        self,
        *,
        turn_index: int,
        current_request: str,
        latest_tool_result: str = "",
        session_state: str = "",
        recent_conversation: str = "",
        instructions_chars: int = 0,
    ) -> None:
        working_memory, working_ids = self._section_payload("working")
        source_memory, source_ids = self._section_payload("source")
        workspace_memory, workspace_ids = self._section_payload("workspace")
        sections = [
            {"name": "Current request", "chars": len(current_request), "entry_ids": []},
            {"name": "Latest tool result", "chars": len(latest_tool_result), "entry_ids": []},
            {"name": "Working memory", "chars": len(working_memory), "entry_ids": working_ids},
            {"name": "Known sources", "chars": len(source_memory), "entry_ids": source_ids},
            {"name": "Workspace memory", "chars": len(workspace_memory), "entry_ids": workspace_ids},
            {"name": "Session state", "chars": len(session_state), "entry_ids": []},
            {"name": "Recent conversation", "chars": len(recent_conversation), "entry_ids": []},
        ]
        self.context_turns.append(
            {
                "turn_index": turn_index,
                "sections": sections,
            }
        )
        self.budget_turns.append(
            {
                "turn_index": turn_index,
                "instructions_chars": instructions_chars,
                "current_request_chars": len(current_request),
                "latest_tool_result_chars": len(latest_tool_result),
                "working_memory_chars": len(working_memory),
                "known_sources_chars": len(source_memory),
                "workspace_memory_chars": len(workspace_memory),
                "session_state_chars": len(session_state),
                "recent_conversation_chars": len(recent_conversation),
                "estimated_total_tokens": _estimate_tokens(
                    "\n".join(
                        [
                            "x" * instructions_chars,
                            current_request,
                            latest_tool_result,
                            working_memory,
                            source_memory,
                            workspace_memory,
                            session_state,
                            recent_conversation,
                        ]
                    )
                ),
            }
        )
        self.flush_artifacts()

    def memory_status(self) -> dict:
        counts = {
            "task": 0,
            "source": 0,
            "workspace": 0,
            "execution": 0,
            "conversation": 0,
        }
        for entry in self.entries.values():
            kind = entry.get("kind")
            if kind in counts:
                counts[kind] += 1
        budget = self.budget_turns[-1] if self.budget_turns else {
            "instructions_chars": 0,
            "current_request_chars": 0,
            "latest_tool_result_chars": 0,
            "working_memory_chars": 0,
            "known_sources_chars": 0,
            "workspace_memory_chars": 0,
            "session_state_chars": 0,
            "recent_conversation_chars": 0,
            "estimated_total_tokens": 0,
        }
        budget = {k: v for k, v in budget.items() if k != "turn_index"}
        return {
            "ok": True,
            "counts": counts,
            "budget": budget,
            "checkpoint_available": bool(self.checkpoints),
        }

    def save_checkpoint(self, label: str, turn_index: int = 0) -> dict:
        checkpoint_id = self._alloc_checkpoint_id()
        entries = [copy.deepcopy(self.entries[entry_id]) for entry_id in self.entry_order]
        self._checkpoint_payloads[checkpoint_id] = entries
        checkpoint = {
            "checkpoint_id": checkpoint_id,
            "label": label,
            "entries": [entry["id"] for entry in entries],
            "saved_turn": turn_index,
        }
        self.checkpoints.append(checkpoint)
        self.events.append(
            {
                "turn_index": turn_index,
                "event": "checkpoint_saved",
                "checkpoint_id": checkpoint_id,
            }
        )
        self.flush_artifacts()
        return {"ok": True, "checkpoint_id": checkpoint_id, "entries": len(entries)}

    def resume_checkpoint(self, checkpoint_id: str, turn_index: int = 0) -> dict:
        payload = self._checkpoint_payloads.get(checkpoint_id)
        if payload is None:
            return self._error(
                "unknown_checkpoint_id",
                "checkpoint was not found",
                checkpoint_id=checkpoint_id,
            )
        self.entries = {}
        self.entry_order = []
        for entry in copy.deepcopy(payload):
            entry_id = str(entry["id"])
            self.entries[entry_id] = entry
            self.entry_order.append(entry_id)
        for checkpoint in self.checkpoints:
            if checkpoint.get("checkpoint_id") == checkpoint_id:
                checkpoint["resumed_turn"] = turn_index
                break
        self.events.append(
            {
                "turn_index": turn_index,
                "event": "checkpoint_resumed",
                "checkpoint_id": checkpoint_id,
            }
        )
        self.flush_artifacts()
        return {
            "ok": True,
            "checkpoint_id": checkpoint_id,
            "restored_entries": len(payload),
        }
