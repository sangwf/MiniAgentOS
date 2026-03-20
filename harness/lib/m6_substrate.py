from __future__ import annotations

import atexit
import json
from pathlib import Path
from urllib.parse import urlparse


def _normalize_query(value: str) -> str:
    return " ".join(value.lower().split())


def _content_preview(text: str, limit: int = 512) -> str:
    normalized = " ".join(text.split())
    data = normalized.encode("utf-8")
    if len(data) <= limit:
        return normalized
    clipped = data[:limit]
    while clipped:
        try:
            return clipped.decode("utf-8") + "..."
        except UnicodeDecodeError as exc:
            clipped = clipped[: exc.start]
    return ""


class M6Substrate:
    def __init__(
        self,
        fixture_path: Path | None,
        source_base_url: str | None,
        output_dir: Path | None,
    ) -> None:
        self.fixture_path = fixture_path.resolve() if fixture_path is not None else None
        self.source_base_url = (source_base_url or "").rstrip("/")
        self.output_dir = output_dir.resolve() if output_dir is not None else None
        self.searches: list[dict] = []
        self.fetched_sources: list[dict] = []
        self.source_memory: dict[str, dict] = {}
        self._fixture = self._load_fixture()
        atexit.register(self.flush_artifacts)

    def _load_fixture(self) -> dict:
        if self.fixture_path is None or not self.fixture_path.exists():
            return {}
        return json.loads(self.fixture_path.read_text(encoding="utf-8"))

    def available(self) -> bool:
        return bool(self._fixture)

    def _write_artifact(self, name: str, payload) -> None:
        if self.output_dir is None:
            return
        (self.output_dir / name).write_text(
            json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )

    def flush_artifacts(self) -> None:
        self._write_artifact("search_results.json", {"searches": self.searches})
        self._write_artifact("fetched_sources.json", {"sources": self.fetched_sources})
        self._write_artifact(
            "source_memory.json",
            {"sources": sorted(self.source_memory.values(), key=lambda item: item.get("id", ""))},
        )

    def _error(self, code: str, message: str) -> dict:
        return {
            "ok": False,
            "error": {
                "code": code,
                "message": message,
            },
        }

    def _build_url(self, item: dict) -> str:
        url = str(item.get("url", "")).strip()
        if url:
            return url
        source_path = str(item.get("source_path", "")).strip()
        if not source_path:
            return ""
        if source_path.startswith("http://") or source_path.startswith("https://"):
            return source_path
        if not source_path.startswith("/"):
            source_path = "/" + source_path
        return self.source_base_url + source_path

    def _remember_result(self, result: dict, turn_index: int) -> None:
        key = result["id"]
        record = self.source_memory.get(key)
        if record is None:
            record = {
                "id": result["id"],
                "url": result["url"],
                "domain": result["domain"],
                "from_search": True,
                "fetched": False,
                "first_seen_turn": turn_index,
                "last_used_turn": turn_index,
            }
            self.source_memory[key] = record
        else:
            record["last_used_turn"] = turn_index

    def search_web(
        self,
        query: str,
        *,
        top_k: int = 5,
        freshness: str | None = None,
        domain_allowlist: list[str] | None = None,
        domain_denylist: list[str] | None = None,
        locale: str | None = None,
        turn_index: int = 0,
        tool_call_index: int = 0,
    ) -> dict:
        domain_allowlist = list(domain_allowlist or [])
        domain_denylist = list(domain_denylist or [])
        if len(query.encode("utf-8")) > 512:
            return self._error("policy_denied", "query length exceeds policy limit")
        if top_k < 1 or top_k > 10:
            return self._error("policy_denied", "top_k exceeds policy limit")
        if len(domain_allowlist) > 10 or len(domain_denylist) > 10:
            return self._error("policy_denied", "domain filter exceeds policy limit")
        if not self.available():
            return self._error("backend_unavailable", "search backend is not configured")

        queries = self._fixture.get("queries", {})
        raw_results = queries.get(_normalize_query(query), [])
        normalized_results: list[dict] = []
        for index, raw in enumerate(raw_results[:top_k], start=1):
            url = self._build_url(raw)
            if not url:
                continue
            domain = str(raw.get("domain") or urlparse(url).netloc)
            if domain_allowlist and domain not in domain_allowlist:
                continue
            if domain_denylist and domain in domain_denylist:
                continue
            snippet = str(raw.get("snippet", ""))
            normalized = {
                "id": str(raw.get("id", f"r{index}")),
                "title": str(raw.get("title", "")),
                "url": url,
                "snippet": snippet[:512],
                "domain": domain,
                "rank": int(raw.get("rank", index)),
            }
            if raw.get("published_at"):
                normalized["published_at"] = str(raw["published_at"])
            normalized_results.append(normalized)
            self._remember_result(normalized, turn_index)

        record = {
            "turn_index": turn_index,
            "tool_call_index": tool_call_index,
            "query": query,
            "top_k": top_k,
            "freshness": freshness,
            "locale": locale,
            "results": normalized_results,
            "truncated": len(raw_results) > len(normalized_results),
        }
        self.searches.append(record)
        self.flush_artifacts()
        return {
            "ok": True,
            "query": query,
            "results": normalized_results,
            "truncated": record["truncated"],
            "provider": "fixture",
        }

    def record_fetch(
        self,
        *,
        url: str,
        content: str,
        turn_index: int,
        tool_call_index: int,
        search_result_id: str | None = None,
        status_code: int = 200,
        content_type: str = "text/markdown; charset=utf-8",
    ) -> None:
        self.fetched_sources.append(
            {
                "turn_index": turn_index,
                "tool_call_index": tool_call_index,
                "search_result_id": search_result_id,
                "url": url,
                "status_code": status_code,
                "content_type": content_type,
                "content_preview": _content_preview(content),
                "truncated": False,
            }
        )
        if search_result_id:
            record = self.source_memory.get(search_result_id)
            if record is not None:
                record["fetched"] = True
                record["last_used_turn"] = turn_index
        self.flush_artifacts()

    def mark_used(self, source_ids: list[str], turn_index: int) -> None:
        updated = False
        for source_id in source_ids:
            record = self.source_memory.get(source_id)
            if record is None:
                continue
            record["last_used_turn"] = turn_index
            updated = True
        if updated:
            self.flush_artifacts()
