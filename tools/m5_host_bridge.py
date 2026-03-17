#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import os
import subprocess
import time
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from threading import Lock
from urllib.parse import parse_qs, urlparse

PROCESS_TIMEOUT_DEFAULT_SEC = 5
PROCESS_TIMEOUT_MAX_SEC = 30
PROCESS_OUTPUT_LIMIT_BYTES = 2048
OPENAI_RESPONSES_URL = "https://api.openai.com/v1/responses"
OPENAI_RELAY_TIMEOUT_SEC = 60


def upstream_status_from_headers(header_blob: str) -> int:
    status = 502
    for line in header_blob.splitlines():
        if not line.startswith("HTTP/"):
            continue
        parts = line.split()
        if len(parts) >= 2 and parts[1].isdigit():
            status = int(parts[1])
    return status


def relay_openai_responses(raw_body: bytes) -> tuple[int, bytes]:
    api_key = os.environ.get("OPENAI_API_KEY", "").strip()
    if not api_key:
        raise RuntimeError("OPENAI_API_KEY is not set for the host bridge")
    command = [
        "curl",
        "-sS",
        "--http1.1",
        "--max-time",
        str(OPENAI_RELAY_TIMEOUT_SEC),
        "-o",
        "-",
        "-D",
        "-",
        "-X",
        "POST",
        OPENAI_RESPONSES_URL,
        "-H",
        f"Authorization: Bearer {api_key}",
        "-H",
        "Content-Type: application/json",
        "--data-binary",
        "@-",
    ]
    if os.environ.get("MINIOS_USE_HOST_SOCKS5_PROXY", "").strip().lower() in {"1", "true", "yes"}:
        proxy_port = os.environ.get("MINIOS_HOST_SOCKS5_PORT", "10808").strip() or "10808"
        command[1:1] = ["--socks5-hostname", f"127.0.0.1:{proxy_port}"]
    completed = subprocess.run(
        command,
        input=raw_body,
        capture_output=True,
        check=False,
        timeout=OPENAI_RELAY_TIMEOUT_SEC + 5,
    )
    if completed.returncode != 0:
        detail = coerce_text(completed.stderr).strip() or "curl relay failed"
        raise RuntimeError(detail)
    raw_output = completed.stdout
    status = 502
    cursor = raw_output
    while cursor.startswith(b"HTTP/"):
        header_end = cursor.find(b"\r\n\r\n")
        if header_end == -1:
            raise RuntimeError("relay response was missing HTTP headers")
        header_blob = cursor[:header_end].decode("iso-8859-1", errors="replace")
        status = upstream_status_from_headers(header_blob)
        cursor = cursor[header_end + 4 :]
        if 100 <= status < 200:
            continue
        return status, cursor
    raise RuntimeError("relay response did not begin with an HTTP status line")


def resolve_workspace_path(workspace_root: Path, raw_path: str) -> Path:
    if not raw_path:
        return workspace_root
    rel = Path(raw_path)
    if rel.is_absolute() or any(part in {"..", "."} for part in rel.parts):
        raise ValueError("invalid workspace path")
    resolved = (workspace_root / rel).resolve()
    if workspace_root not in {resolved, *resolved.parents}:
        raise ValueError("invalid workspace path")
    return resolved


def truncate_utf8(text: str, limit_bytes: int) -> tuple[str, bool]:
    data = text.encode("utf-8")
    if len(data) <= limit_bytes:
        return text, False
    clipped = data[:limit_bytes]
    while clipped:
        try:
            return clipped.decode("utf-8"), True
        except UnicodeDecodeError as exc:
            clipped = clipped[: exc.start]
    return "", True


def coerce_text(value: str | bytes | None) -> str:
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return value


class ArtifactRecorder:
    def __init__(self, output_dir: Path | None) -> None:
        self.output_dir = output_dir.resolve() if output_dir is not None else None
        self._lock = Lock()
        self.file_reads: list[dict] = []
        self.file_writes: list[dict] = []
        self.file_patches: list[dict] = []
        self.tool_errors: list[dict] = []
        self.process_runs: list[dict] = []
        self.process_outputs: dict[str, dict] = {}

    def _write_json(self, name: str, payload) -> None:
        if self.output_dir is None:
            return
        self.output_dir.mkdir(parents=True, exist_ok=True)
        (self.output_dir / name).write_text(
            json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )

    def _flush_locked(self) -> None:
        if self.output_dir is None:
            return
        self._write_json("file_reads.json", self.file_reads)
        self._write_json("file_writes.json", self.file_writes)
        self._write_json("file_patches.json", self.file_patches)
        self._write_json("tool_errors.json", self.tool_errors)
        self._write_json("process_runs.json", self.process_runs)
        process_output_dir = self.output_dir / "process_output"
        process_output_dir.mkdir(exist_ok=True)
        for process_id, payload in self.process_outputs.items():
            (process_output_dir / f"{process_id}.stdout").write_text(
                payload.get("stdout", ""),
                encoding="utf-8",
            )
            (process_output_dir / f"{process_id}.stderr").write_text(
                payload.get("stderr", ""),
                encoding="utf-8",
            )

    def record_file_read(self, path: str, offset: int, limit: int) -> None:
        with self._lock:
            self.file_reads.append({"path": path, "offset": offset, "limit": limit})
            self._flush_locked()

    def record_file_write(self, path: str, bytes_written: int) -> None:
        with self._lock:
            self.file_writes.append({"path": path, "bytes_written": bytes_written})
            self._flush_locked()

    def record_file_patch(
        self,
        files_changed: list[str],
        created_files: list[str],
        deleted_files: list[str],
    ) -> None:
        with self._lock:
            self.file_patches.append(
                {
                    "files_changed": files_changed,
                    "created_files": created_files,
                    "deleted_files": deleted_files,
                }
            )
            self._flush_locked()

    def record_tool_error(self, code: str, message: str, **extra) -> None:
        with self._lock:
            payload = {"code": code, "message": message}
            payload.update(extra)
            self.tool_errors.append(payload)
            self._flush_locked()

    def record_process_run(self, record: dict, stdout: str, stderr: str) -> None:
        process_id = str(record.get("process_id", ""))
        with self._lock:
            self.process_runs.append(record)
            if process_id:
                self.process_outputs[process_id] = {
                    "stdout": stdout,
                    "stderr": stderr,
                }
            self._flush_locked()


class ProcessStore:
    def __init__(
        self,
        workspace_root: Path,
        python_image: str,
        artifacts: ArtifactRecorder | None = None,
    ) -> None:
        self.workspace_root = workspace_root
        self.python_image = python_image
        self.artifacts = artifacts
        self._lock = Lock()
        self._next_process_id = 1
        self._runs: dict[str, dict] = {}

    def _allocate_process_id(self) -> str:
        with self._lock:
            process_id = str(self._next_process_id)
            self._next_process_id += 1
        return process_id

    def _save_run(self, process_id: str, payload: dict) -> None:
        with self._lock:
            self._runs[process_id] = payload

    def get_output(self, process_id: str, offset: int = 0, limit: int = 8192) -> dict:
        with self._lock:
            payload = self._runs.get(process_id)
        if payload is None:
            raise LookupError("process run was not found")
        stdout = payload["stdout"][offset : offset + limit]
        stderr = payload["stderr"][offset : offset + limit]
        next_offset = offset + max(len(stdout), len(stderr))
        return {
            "ok": True,
            "process_id": process_id,
            "status": payload["status"],
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": payload["exit_code"],
            "next_offset": next_offset,
            "truncated": next_offset
            < max(len(payload["stdout"]), len(payload["stderr"])),
        }

    def run_python(self, raw_path: str, timeout_sec: int) -> dict:
        file_path = resolve_workspace_path(self.workspace_root, raw_path)
        if not file_path.exists():
            raise FileNotFoundError(raw_path)
        if not file_path.is_file():
            raise ValueError("path must reference a file")
        process_id = self._allocate_process_id()
        rel_path = file_path.relative_to(self.workspace_root).as_posix()
        timeout_sec = max(1, min(int(timeout_sec), PROCESS_TIMEOUT_MAX_SEC))
        command = [
            "docker",
            "run",
            "--rm",
            "--network",
            "none",
            "-v",
            f"{self.workspace_root}:/workspace",
            "-w",
            "/workspace",
            self.python_image,
            "python3",
            rel_path,
        ]
        started_at = time.time()
        try:
            completed = subprocess.run(
                command,
                capture_output=True,
                text=True,
                encoding="utf-8",
                timeout=timeout_sec,
                check=False,
            )
            stdout, stdout_truncated = truncate_utf8(completed.stdout, PROCESS_OUTPUT_LIMIT_BYTES)
            stderr, stderr_truncated = truncate_utf8(completed.stderr, PROCESS_OUTPUT_LIMIT_BYTES)
            payload = {
                "status": "exited",
                "exit_code": completed.returncode,
                "stdout": stdout,
                "stderr": stderr,
            }
            timed_out = False
        except subprocess.TimeoutExpired as exc:
            stdout, stdout_truncated = truncate_utf8(coerce_text(exc.stdout), PROCESS_OUTPUT_LIMIT_BYTES)
            stderr, stderr_truncated = truncate_utf8(coerce_text(exc.stderr), PROCESS_OUTPUT_LIMIT_BYTES)
            payload = {
                "status": "timed_out",
                "exit_code": 124,
                "stdout": stdout,
                "stderr": stderr,
            }
            timed_out = True
        self._save_run(process_id, payload)
        record = {
            "process_id": process_id,
            "argv": ["python3", rel_path],
            "cwd": "",
            "profile": "python",
            "status": payload["status"],
            "exit_code": payload["exit_code"],
            "stdout_bytes": len(payload["stdout"].encode("utf-8")),
            "stderr_bytes": len(payload["stderr"].encode("utf-8")),
            "timed_out": timed_out,
            "duration_ms": int((time.time() - started_at) * 1000),
        }
        if self.artifacts is not None:
            self.artifacts.record_process_run(record, payload["stdout"], payload["stderr"])
        return {
            "ok": True,
            "process_id": process_id,
            "status": "running",
        }


def list_workspace(workspace_root: Path, raw_path: str, depth: int) -> dict:
    root = resolve_workspace_path(workspace_root, raw_path)
    entries = []
    for child in sorted(root.rglob("*")):
        rel = child.relative_to(workspace_root).as_posix()
        rel_depth = len(Path(rel).parts) - len(Path(raw_path).parts) if raw_path else len(Path(rel).parts)
        if rel_depth > depth:
            continue
        entry = {"path": rel, "kind": "dir" if child.is_dir() else "file"}
        if child.is_file():
            entry["size"] = child.stat().st_size
        entries.append(entry)
    return {"ok": True, "path": raw_path, "entries": entries, "truncated": False}


def read_file(workspace_root: Path, raw_path: str, offset: int, limit: int) -> dict:
    file_path = resolve_workspace_path(workspace_root, raw_path)
    content = file_path.read_text(encoding="utf-8")
    segment = content[offset : offset + limit]
    return {
        "ok": True,
        "path": raw_path,
        "content": segment,
        "offset": offset,
        "bytes_read": len(segment.encode("utf-8")),
        "eof": offset + len(segment) >= len(content),
        "truncated": offset + len(segment) < len(content),
    }


def write_file(
    workspace_root: Path,
    raw_path: str,
    content: str,
    *,
    create: bool = True,
    overwrite: bool = True,
) -> dict:
    file_path = resolve_workspace_path(workspace_root, raw_path)
    existed = file_path.exists()
    if existed and not overwrite:
        raise ValueError("overwrite disabled")
    if not existed and not create:
        raise ValueError("create disabled")
    file_path.parent.mkdir(parents=True, exist_ok=True)
    file_path.write_text(content, encoding="utf-8")
    return {
        "ok": True,
        "path": raw_path,
        "bytes_written": len(content.encode("utf-8")),
        "created": not existed,
    }


def find_sequence(haystack: list[str], needle: list[str], start_index: int) -> int:
    if not needle:
        return start_index
    limit = len(haystack) - len(needle)
    for index in range(start_index, limit + 1):
        if haystack[index : index + len(needle)] == needle:
            return index
    raise ValueError("patch hunk did not match target file")


def join_lines(lines: list[str], trailing_newline: bool) -> str:
    if not lines:
        return ""
    joined = "\n".join(lines)
    if trailing_newline:
        joined += "\n"
    return joined


def apply_update_patch(workspace_root: Path, raw_path: str, body_lines: list[str]) -> dict:
    file_path = resolve_workspace_path(workspace_root, raw_path)
    original_text = file_path.read_text(encoding="utf-8")
    original_lines = original_text.splitlines()
    trailing_newline = original_text.endswith("\n")
    new_lines: list[str] = []
    scan_index = 0
    hunk_lines: list[str] = []
    saw_hunk = False

    def flush_hunk() -> None:
        nonlocal scan_index, saw_hunk
        if not hunk_lines:
            return
        old_chunk = [line[1:] for line in hunk_lines if line.startswith((" ", "-"))]
        new_chunk = [line[1:] for line in hunk_lines if line.startswith((" ", "+"))]
        match_index = find_sequence(original_lines, old_chunk, scan_index)
        new_lines.extend(original_lines[scan_index:match_index])
        new_lines.extend(new_chunk)
        scan_index = match_index + len(old_chunk)
        saw_hunk = True

    for line in body_lines:
        if line.startswith("@@"):
            flush_hunk()
            hunk_lines = []
            continue
        if not line or line[0] not in {" ", "+", "-"}:
            raise ValueError("unsupported patch line in update hunk")
        hunk_lines.append(line)

    flush_hunk()
    if not saw_hunk:
        raise ValueError("update patch did not contain a hunk")
    new_lines.extend(original_lines[scan_index:])
    updated_text = join_lines(new_lines, trailing_newline)
    file_path.write_text(updated_text, encoding="utf-8")
    return {
        "path": raw_path,
        "bytes_written": len(updated_text.encode("utf-8")),
    }


def apply_add_patch(workspace_root: Path, raw_path: str, body_lines: list[str]) -> dict:
    file_path = resolve_workspace_path(workspace_root, raw_path)
    if file_path.exists():
        raise ValueError("add patch target already exists")
    added_lines: list[str] = []
    for line in body_lines:
        if line.startswith("@@"):
            continue
        if not line.startswith("+"):
            raise ValueError("add patch can only contain added lines")
        added_lines.append(line[1:])
    content = join_lines(added_lines, bool(added_lines))
    file_path.parent.mkdir(parents=True, exist_ok=True)
    file_path.write_text(content, encoding="utf-8")
    return {
        "path": raw_path,
        "bytes_written": len(content.encode("utf-8")),
    }


def apply_delete_patch(workspace_root: Path, raw_path: str) -> None:
    file_path = resolve_workspace_path(workspace_root, raw_path)
    if not file_path.exists():
        raise ValueError("delete patch target does not exist")
    file_path.unlink()


def apply_patch(workspace_root: Path, patch: str) -> dict:
    if len(patch.encode("utf-8")) > 65536:
        raise ValueError("patch exceeds size limit")
    lines = patch.splitlines()
    if not lines or lines[0] != "*** Begin Patch" or lines[-1] != "*** End Patch":
        raise ValueError("invalid patch envelope")

    index = 1
    files_changed: list[str] = []
    created_files: list[str] = []
    deleted_files: list[str] = []

    while index < len(lines) - 1:
        line = lines[index]
        if line.startswith("*** Update File: "):
            raw_path = line.split(": ", 1)[1]
            index += 1
            body_start = index
            while index < len(lines) - 1 and not lines[index].startswith("*** "):
                index += 1
            apply_update_patch(workspace_root, raw_path, lines[body_start:index])
            files_changed.append(raw_path)
            continue
        if line.startswith("*** Add File: "):
            raw_path = line.split(": ", 1)[1]
            index += 1
            body_start = index
            while index < len(lines) - 1 and not lines[index].startswith("*** "):
                index += 1
            apply_add_patch(workspace_root, raw_path, lines[body_start:index])
            files_changed.append(raw_path)
            created_files.append(raw_path)
            continue
        if line.startswith("*** Delete File: "):
            raw_path = line.split(": ", 1)[1]
            apply_delete_patch(workspace_root, raw_path)
            files_changed.append(raw_path)
            deleted_files.append(raw_path)
            index += 1
            continue
        raise ValueError("unsupported patch operation")

    return {
        "ok": True,
        "files_changed": files_changed,
        "created_files": created_files,
        "deleted_files": deleted_files,
    }


class M5BridgeHandler(BaseHTTPRequestHandler):
    workspace_root: Path
    process_store: ProcessStore
    artifacts: ArtifactRecorder | None

    def log_message(self, fmt: str, *args) -> None:
        return

    def _record_error(self, code: str, message: str, **extra) -> None:
        if self.artifacts is not None:
            self.artifacts.record_tool_error(code, message, **extra)

    def _write_json(self, status: HTTPStatus, payload: dict) -> None:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _write_raw_json(self, status_code: int, body: bytes) -> None:
        self.send_response(status_code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:
        parsed = urlparse(self.path)
        query = parse_qs(parsed.query, keep_blank_values=True)
        try:
            if parsed.path == "/healthz":
                self._write_json(
                    HTTPStatus.OK,
                    {
                        "ok": True,
                        "workspace_root": str(self.workspace_root),
                        "python_image": self.process_store.python_image,
                    },
                )
                return
            if parsed.path == "/workspace/list":
                raw_path = query.get("path", [""])[0]
                depth = int(query.get("depth", ["3"])[0])
                self._write_json(HTTPStatus.OK, list_workspace(self.workspace_root, raw_path, depth))
                return
            if parsed.path == "/workspace/read":
                raw_path = query.get("path", [""])[0]
                offset = int(query.get("offset", ["0"])[0])
                limit = int(query.get("limit", ["4096"])[0])
                payload = read_file(self.workspace_root, raw_path, offset, limit)
                if self.artifacts is not None:
                    self.artifacts.record_file_read(raw_path, offset, limit)
                self._write_json(HTTPStatus.OK, payload)
                return
            if parsed.path == "/process/output":
                process_id = query.get("id", [""])[0]
                if not process_id:
                    raise ValueError("process id is required")
                offset = int(query.get("offset", ["0"])[0])
                limit = int(query.get("limit", ["8192"])[0])
                self._write_json(
                    HTTPStatus.OK,
                    self.process_store.get_output(process_id, offset=offset, limit=limit),
                )
                return
            self._record_error("not_found", "route not found", route=parsed.path)
            self._write_json(HTTPStatus.NOT_FOUND, {"ok": False, "error": {"code": "not_found", "message": "route not found"}})
        except FileNotFoundError:
            self._record_error("missing_file", "workspace file was not found", route=parsed.path)
            self._write_json(
                HTTPStatus.NOT_FOUND,
                {"ok": False, "error": {"code": "missing_file", "message": "workspace file was not found"}},
            )
        except LookupError:
            self._record_error("missing_process", "process run was not found", route=parsed.path)
            self._write_json(
                HTTPStatus.NOT_FOUND,
                {"ok": False, "error": {"code": "missing_process", "message": "process run was not found"}},
            )
        except ValueError as exc:
            self._record_error("invalid_path", str(exc), route=parsed.path)
            self._write_json(
                HTTPStatus.BAD_REQUEST,
                {"ok": False, "error": {"code": "invalid_path", "message": str(exc)}},
            )

    def do_POST(self) -> None:
        parsed = urlparse(self.path)
        try:
            content_length = int(self.headers.get("Content-Length", "0"))
            raw_body = self.rfile.read(content_length)
            if parsed.path == "/openai/responses":
                status_code, body = relay_openai_responses(raw_body)
                Path("/tmp/minios_last_openai_response.json").write_bytes(body)
                text_preview = ""
                try:
                    parsed_body = json.loads(body.decode("utf-8"))
                    for item in parsed_body.get("output", []):
                        for content in item.get("content", []):
                            if content.get("type") == "output_text":
                                text_preview = content.get("text", "")[:200]
                                raise StopIteration
                except StopIteration:
                    pass
                except Exception:
                    text_preview = ""
                preview = body[:200].decode("utf-8", errors="replace")
                print(
                    f"relay /openai/responses status={status_code} len={len(body)} output_text={b'output_text' in body} text_preview={text_preview!r} preview={preview!r}",
                    flush=True,
                )
                self._write_raw_json(status_code, body)
                return
            payload = json.loads(raw_body.decode("utf-8") or "{}")
            if parsed.path == "/workspace/write":
                raw_path = payload.get("path", "")
                content = payload.get("content", "")
                create = bool(payload.get("create", True))
                overwrite = bool(payload.get("overwrite", True))
                if not isinstance(raw_path, str) or not isinstance(content, str):
                    raise ValueError("path and content must be strings")
                self._write_json(
                    HTTPStatus.OK,
                    write_file(
                        self.workspace_root,
                        raw_path,
                        content,
                        create=create,
                        overwrite=overwrite,
                    ),
                )
                if self.artifacts is not None:
                    self.artifacts.record_file_write(raw_path, len(content.encode("utf-8")))
                return
            if parsed.path == "/workspace/apply-patch":
                patch = payload.get("patch", "")
                if not isinstance(patch, str):
                    raise ValueError("patch must be a string")
                patch_result = apply_patch(self.workspace_root, patch)
                if self.artifacts is not None:
                    self.artifacts.record_file_patch(
                        patch_result.get("files_changed", []),
                        patch_result.get("created_files", []),
                        patch_result.get("deleted_files", []),
                    )
                    deleted_files = set(patch_result.get("deleted_files", []))
                    for raw_path in patch_result.get("files_changed", []):
                        if raw_path in deleted_files:
                            continue
                        file_path = resolve_workspace_path(self.workspace_root, raw_path)
                        self.artifacts.record_file_write(raw_path, file_path.stat().st_size)
                self._write_json(
                    HTTPStatus.OK,
                    patch_result,
                )
                return
            if parsed.path == "/process/run-python":
                raw_path = payload.get("path", "")
                timeout_sec = int(payload.get("timeout_sec", PROCESS_TIMEOUT_DEFAULT_SEC))
                if not isinstance(raw_path, str):
                    raise ValueError("path must be a string")
                self._write_json(
                    HTTPStatus.OK,
                    self.process_store.run_python(raw_path, timeout_sec),
                )
                return
            self._write_json(
                HTTPStatus.NOT_FOUND,
                {"ok": False, "error": {"code": "not_found", "message": "route not found"}},
            )
        except json.JSONDecodeError:
            self._record_error("invalid_json", "request body was not valid JSON", route=parsed.path)
            self._write_json(
                HTTPStatus.BAD_REQUEST,
                {"ok": False, "error": {"code": "invalid_json", "message": "request body was not valid JSON"}},
            )
        except ValueError as exc:
            code = "invalid_path" if str(exc) == "invalid workspace path" else "invalid_request"
            self._record_error(code, str(exc), route=parsed.path)
            self._write_json(
                HTTPStatus.BAD_REQUEST,
                {"ok": False, "error": {"code": code, "message": str(exc)}},
            )
        except RuntimeError as exc:
            self._record_error("relay_failed", str(exc), route=parsed.path)
            self._write_json(
                HTTPStatus.BAD_GATEWAY,
                {"ok": False, "error": {"code": "relay_failed", "message": str(exc)}},
            )


def main() -> int:
    parser = argparse.ArgumentParser(description="MiniAgentOS M5 host bridge")
    parser.add_argument("--workspace", required=True, help="Workspace root exposed to the guest")
    parser.add_argument("--bind", default="0.0.0.0", help="Bind address")
    parser.add_argument("--port", type=int, default=8090, help="Bind port")
    parser.add_argument("--python-image", default="python:3.12-slim", help="Docker image used for Python process runs")
    parser.add_argument("--output-dir", help="Optional output directory for harness artifacts")
    args = parser.parse_args()

    workspace_root = Path(args.workspace).resolve()
    if not workspace_root.is_dir():
        raise SystemExit(f"workspace root is not a directory: {workspace_root}")
    output_dir = Path(args.output_dir).resolve() if args.output_dir else None
    artifacts = ArtifactRecorder(output_dir)
    process_store = ProcessStore(workspace_root, args.python_image, artifacts=artifacts)

    handler = type(
        "BoundM5BridgeHandler",
        (M5BridgeHandler,),
        {
            "workspace_root": workspace_root,
            "process_store": process_store,
            "artifacts": artifacts,
        },
    )
    server = ThreadingHTTPServer((args.bind, args.port), handler)
    print(f"MiniAgentOS M5 bridge listening on http://{args.bind}:{args.port} workspace={workspace_root}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
