from __future__ import annotations

import atexit
import json
from pathlib import Path
import subprocess
import time


class M5Substrate:
    def __init__(self, workspace_root: Path | None, output_dir: Path | None, docker_image: str):
        self.workspace_root = workspace_root.resolve() if workspace_root is not None else None
        self.output_dir = output_dir.resolve() if output_dir is not None else None
        self.docker_image = docker_image
        self.file_reads: list[dict] = []
        self.file_writes: list[dict] = []
        self.file_patches: list[dict] = []
        self.tool_errors: list[dict] = []
        self.process_runs: list[dict] = []
        self.process_outputs: dict[str, dict] = {}
        self._next_process_id = 1
        atexit.register(self.flush_artifacts)

    def available(self) -> bool:
        return self.workspace_root is not None

    def _write_artifact(self, name: str, payload):
        if self.output_dir is None:
            return
        (self.output_dir / name).write_text(
            json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )

    def flush_artifacts(self):
        self._write_artifact("file_reads.json", self.file_reads)
        self._write_artifact("file_writes.json", self.file_writes)
        self._write_artifact("file_patches.json", self.file_patches)
        self._write_artifact("tool_errors.json", self.tool_errors)
        self._write_artifact("process_runs.json", self.process_runs)
        if self.output_dir is None:
            return
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

    def workspace_path(self, path: str) -> Path:
        if self.workspace_root is None:
            raise ValueError("workspace unavailable")
        if not path:
            return self.workspace_root
        rel = Path(path)
        if rel.is_absolute() or any(part in {"..", "."} for part in rel.parts):
            raise ValueError("invalid workspace path")
        resolved = (self.workspace_root / rel).resolve()
        if self.workspace_root not in {resolved, *resolved.parents}:
            raise ValueError("path escaped workspace root")
        return resolved

    def _error(self, code: str, message: str, **extra) -> dict:
        payload = {
            "ok": False,
            "error": {
                "code": code,
                "message": message,
            },
        }
        payload.update(extra)
        self.tool_errors.append(
            {
                "code": code,
                "message": message,
                **extra,
            }
        )
        self.flush_artifacts()
        return payload

    def list_workspace(self, path: str = "", depth: int = 2) -> dict:
        try:
            root = self.workspace_path(path)
        except ValueError as exc:
            return self._error("invalid_path", str(exc), path=path)
        entries = []
        for child in sorted(root.rglob("*")):
            rel = child.relative_to(self.workspace_root).as_posix()
            rel_depth = len(Path(rel).parts) - len(Path(path).parts) if path else len(Path(rel).parts)
            if rel_depth > depth:
                continue
            entry = {"path": rel, "kind": "dir" if child.is_dir() else "file"}
            if child.is_file():
                entry["size"] = child.stat().st_size
            entries.append(entry)
        return {"ok": True, "path": path, "entries": entries, "truncated": False}

    def read_file(self, path: str, offset: int = 0, limit: int = 4096) -> dict:
        try:
            file_path = self.workspace_path(path)
        except ValueError as exc:
            return self._error("invalid_path", str(exc), path=path, offset=offset, limit=limit)
        try:
            content = file_path.read_text(encoding="utf-8")
        except FileNotFoundError:
            return self._error("missing_file", "workspace file was not found", path=path)
        segment = content[offset : offset + limit]
        result = {
            "ok": True,
            "path": path,
            "content": segment,
            "offset": offset,
            "bytes_read": len(segment.encode("utf-8")),
            "eof": offset + len(segment) >= len(content),
            "truncated": offset + len(segment) < len(content),
        }
        self.file_reads.append({"path": path, "offset": offset, "limit": limit})
        self.flush_artifacts()
        return result

    def write_file(self, path: str, content: str, create: bool = True, overwrite: bool = True) -> dict:
        try:
            file_path = self.workspace_path(path)
        except ValueError as exc:
            return self._error(
                "invalid_path",
                str(exc),
                path=path,
                create=create,
                overwrite=overwrite,
            )
        file_path.parent.mkdir(parents=True, exist_ok=True)
        existed = file_path.exists()
        if existed and not overwrite:
            return self._error(
                "overwrite_denied",
                "overwrite disabled",
                path=path,
                create=create,
                overwrite=overwrite,
            )
        if not existed and not create:
            return self._error(
                "create_denied",
                "create disabled",
                path=path,
                create=create,
                overwrite=overwrite,
            )
        file_path.write_text(content, encoding="utf-8")
        result = {
            "ok": True,
            "path": path,
            "bytes_written": len(content.encode("utf-8")),
            "created": not existed,
        }
        self.file_writes.append({"path": path, "bytes_written": result["bytes_written"]})
        self.flush_artifacts()
        return result

    def _find_sequence(self, haystack: list[str], needle: list[str], start_index: int) -> int:
        if not needle:
            return start_index
        limit = len(haystack) - len(needle)
        for index in range(start_index, limit + 1):
            if haystack[index : index + len(needle)] == needle:
                return index
        raise ValueError("patch hunk did not match target file")

    def _join_lines(self, lines: list[str], trailing_newline: bool) -> str:
        if not lines:
            return ""
        joined = "\n".join(lines)
        if trailing_newline:
            joined += "\n"
        return joined

    def _apply_update_patch(self, path: str, body_lines: list[str]) -> dict:
        file_path = self.workspace_path(path)
        original_text = file_path.read_text(encoding="utf-8")
        original_lines = original_text.splitlines()
        trailing_newline = original_text.endswith("\n")
        new_lines: list[str] = []
        scan_index = 0
        hunk_lines: list[str] = []
        saw_hunk = False

        def flush_hunk():
            nonlocal scan_index, new_lines, saw_hunk
            if not hunk_lines:
                return
            old_chunk = [line[1:] for line in hunk_lines if line.startswith((" ", "-"))]
            new_chunk = [line[1:] for line in hunk_lines if line.startswith((" ", "+"))]
            match_index = self._find_sequence(original_lines, old_chunk, scan_index)
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
        updated_text = self._join_lines(new_lines, trailing_newline)
        file_path.write_text(updated_text, encoding="utf-8")
        return {
            "path": path,
            "bytes_written": len(updated_text.encode("utf-8")),
        }

    def _apply_add_patch(self, path: str, body_lines: list[str]) -> dict:
        file_path = self.workspace_path(path)
        if file_path.exists():
            raise ValueError("add patch target already exists")
        added_lines: list[str] = []
        for line in body_lines:
            if line.startswith("@@"):
                continue
            if not line.startswith("+"):
                raise ValueError("add patch can only contain added lines")
            added_lines.append(line[1:])
        content = self._join_lines(added_lines, bool(added_lines))
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(content, encoding="utf-8")
        return {
            "path": path,
            "bytes_written": len(content.encode("utf-8")),
        }

    def _apply_delete_patch(self, path: str) -> None:
        file_path = self.workspace_path(path)
        if not file_path.exists():
            raise ValueError("delete patch target does not exist")
        file_path.unlink()

    def apply_patch(self, patch: str) -> dict:
        try:
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
                    path = line.split(": ", 1)[1]
                    index += 1
                    body_start = index
                    while index < len(lines) - 1 and not lines[index].startswith("*** "):
                        index += 1
                    result = self._apply_update_patch(path, lines[body_start:index])
                    files_changed.append(path)
                    self.file_writes.append({"path": path, "bytes_written": result["bytes_written"]})
                    continue
                if line.startswith("*** Add File: "):
                    path = line.split(": ", 1)[1]
                    index += 1
                    body_start = index
                    while index < len(lines) - 1 and not lines[index].startswith("*** "):
                        index += 1
                    result = self._apply_add_patch(path, lines[body_start:index])
                    files_changed.append(path)
                    created_files.append(path)
                    self.file_writes.append({"path": path, "bytes_written": result["bytes_written"]})
                    continue
                if line.startswith("*** Delete File: "):
                    path = line.split(": ", 1)[1]
                    self._apply_delete_patch(path)
                    files_changed.append(path)
                    deleted_files.append(path)
                    index += 1
                    continue
                raise ValueError("unsupported patch operation")

            result = {
                "ok": True,
                "files_changed": files_changed,
                "created_files": created_files,
                "deleted_files": deleted_files,
            }
            self.file_patches.append(
                {
                    "files_changed": files_changed,
                    "created_files": created_files,
                    "deleted_files": deleted_files,
                }
            )
            self.flush_artifacts()
            return result
        except ValueError as exc:
            return self._error("invalid_patch", str(exc))

    def _alloc_process_id(self) -> str:
        process_id = f"proc_{self._next_process_id:04d}"
        self._next_process_id += 1
        return process_id

    def run_process(self, argv: list[str], cwd: str = "", profile: str = "default", timeout_sec: int = 20) -> dict:
        if not argv or argv[0] != "python3":
            return self._error(
                "policy_denied",
                "only python3 is allowed in the first M5 fixture",
                argv=argv,
                cwd=cwd,
                profile=profile,
            )
        if any(arg == "-c" for arg in argv[1:]):
            return self._error(
                "policy_denied",
                "python3 -c is not allowed",
                argv=argv,
                cwd=cwd,
                profile=profile,
            )

        process_id = self._alloc_process_id()
        try:
            workspace_cwd = self.workspace_path(cwd)
        except ValueError as exc:
            return self._error(
                "invalid_path",
                str(exc),
                argv=argv,
                cwd=cwd,
                profile=profile,
            )
        rel_cwd = workspace_cwd.relative_to(self.workspace_root).as_posix() if workspace_cwd != self.workspace_root else ""
        command = [
            "docker",
            "run",
            "--rm",
            "--network",
            "none",
            "-v",
            f"{self.workspace_root}:/workspace",
            "-w",
            f"/workspace/{rel_cwd}" if rel_cwd else "/workspace",
            self.docker_image,
            *argv,
        ]
        started_at = time.time()
        timed_out = False
        try:
            completed = subprocess.run(
                command,
                capture_output=True,
                text=True,
                timeout=timeout_sec,
                check=False,
            )
            exit_code = completed.returncode
            stdout = completed.stdout
            stderr = completed.stderr
        except subprocess.TimeoutExpired as exc:
            timed_out = True
            exit_code = None
            stdout = exc.stdout or ""
            stderr = exc.stderr or ""

        record = {
            "process_id": process_id,
            "argv": argv,
            "cwd": cwd,
            "profile": profile,
            "status": "timed_out" if timed_out else "exited",
            "exit_code": exit_code,
            "stdout_bytes": len(stdout.encode("utf-8")),
            "stderr_bytes": len(stderr.encode("utf-8")),
            "timed_out": timed_out,
            "duration_ms": int((time.time() - started_at) * 1000),
        }
        self.process_runs.append(record)
        self.process_outputs[process_id] = {
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code,
            "status": record["status"],
        }
        self.flush_artifacts()
        return {"ok": True, "process_id": process_id, "status": "running"}

    def read_process_output(self, process_id: str, offset: int = 0, limit: int = 8192) -> dict:
        payload = self.process_outputs.get(process_id)
        if payload is None:
            return self._error(
                "missing_process",
                "process_id was not found",
                process_id=process_id,
                offset=offset,
            )
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
            "truncated": next_offset < max(len(payload["stdout"]), len(payload["stderr"])),
        }
