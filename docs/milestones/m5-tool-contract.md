# M5 Tool Contract Draft

## Purpose

This document defines the first bounded coding tool surface for M5.

The goal is to make the runtime capable of a real:

`inspect -> edit -> execute -> observe -> iterate`

loop without requiring an unrestricted shell or a guest-native general-purpose
filesystem or process model.

## Design Principles

- tools are runtime-owned contracts exposed inside the MiniAgentOS guest
- implementations may be host-backed in the first M5 delivery
- all tool calls must be policy-bounded and trace-visible
- tool arguments should be structured, not shell-language-shaped
- tool results must be machine-readable and compact

## Shared Concepts

### Workspace Root

Every M5 run operates against exactly one bounded workspace root.

The agent never receives an arbitrary host path. It only sees workspace-relative
paths such as:

- `README.md`
- `src/main.py`
- `tests/test_basic.py`

### Path Rules

- paths are UTF-8 text
- paths must be relative
- paths must not escape the workspace root
- `.` and `..` segments are rejected
- symbolic-link behavior is implementation-defined for the first draft and
  should default to rejection if it complicates policy

### Process Model

M5 does not require a general process table. It only requires a bounded process
handle model:

- `run_process` starts one bounded task
- the runtime returns one `process_id`
- `read_process_output` reads status and buffered output for that handle

## Core Tools

### `list_workspace`

List files and directories under a workspace-relative path.

Arguments:

```json
{
  "path": "",
  "depth": 2,
  "include_hidden": false
}
```

Rules:

- `path` defaults to the workspace root when empty
- `depth` is bounded, for example `0..4`
- hidden-file access is policy-controlled

Result:

```json
{
  "ok": true,
  "path": "",
  "entries": [
    { "path": "README.md", "kind": "file", "size": 1821 },
    { "path": "src", "kind": "dir" }
  ],
  "truncated": false
}
```

Failure result example:

```json
{
  "ok": false,
  "error": {
    "code": "invalid_path",
    "message": "invalid workspace path"
  }
}
```

### `read_file`

Read one text file from the workspace.

Arguments:

```json
{
  "path": "src/main.py",
  "offset": 0,
  "limit": 4096
}
```

Rules:

- text files only in the first draft
- `limit` is bounded
- binary or oversized files return a structured refusal or truncation result

Result:

```json
{
  "ok": true,
  "path": "src/main.py",
  "content": "print(\"hello\")\n",
  "offset": 0,
  "bytes_read": 15,
  "eof": true,
  "truncated": false
}
```

### `write_file`

Create or replace one workspace file.

Arguments:

```json
{
  "path": "src/main.py",
  "content": "print(\"hello\")\n",
  "create": true,
  "overwrite": true
}
```

Rules:

- file size is bounded
- directory creation behavior must be explicit
- first draft should prefer whole-file writes, not append semantics

Result:

```json
{
  "ok": true,
  "path": "src/main.py",
  "bytes_written": 15,
  "created": false
}
```

### `apply_patch`

Apply a bounded multi-file patch within the workspace.

Arguments:

```json
{
  "patch": "*** Begin Patch\n*** Update File: src/main.py\n@@\n-print(\"helo\")\n+print(\"hello\")\n*** End Patch\n"
}
```

Rules:

- patch size is bounded
- file count is bounded
- patch must stay within workspace root
- invalid patch application returns structured failure

Result:

```json
{
  "ok": true,
  "files_changed": [
    "src/main.py"
  ],
  "created_files": [],
  "deleted_files": []
}
```

Failure result example:

```json
{
  "ok": false,
  "error": {
    "code": "invalid_patch",
    "message": "patch hunk did not match target file"
  }
}
```

### `run_process`

Start one bounded execution task against the current workspace.

Arguments:

```json
{
  "argv": ["python3", "main.py"],
  "cwd": "",
  "timeout_sec": 20,
  "profile": "default"
}
```

Rules:

- `argv` is structured and required
- shell parsing is not part of the contract
- `cwd` is workspace-relative
- `timeout_sec` is bounded
- `profile` selects a runtime policy profile
- the implementation may use a host-backed sandbox, including Docker

Result:

```json
{
  "ok": true,
  "process_id": "proc_0001",
  "status": "running"
}
```

Failure result example:

```json
{
  "ok": false,
  "error": {
    "code": "policy_denied",
    "message": "python3 -c is not allowed"
  }
}
```

### `read_process_output`

Read buffered output and terminal state for one bounded process handle.

Arguments:

```json
{
  "process_id": "proc_0001",
  "offset": 0,
  "limit": 8192
}
```

Result while running:

```json
{
  "ok": true,
  "process_id": "proc_0001",
  "status": "running",
  "stdout": "starting...\n",
  "stderr": "",
  "exit_code": null,
  "next_offset": 12,
  "truncated": false
}
```

Result after completion:

```json
{
  "ok": true,
  "process_id": "proc_0001",
  "status": "exited",
  "stdout": "Hello\n",
  "stderr": "",
  "exit_code": 0,
  "next_offset": 6,
  "truncated": false
}
```

## Policy Surface

The first M5 implementation should enforce at least:

- one workspace root per run
- path escape rejection
- maximum file size
- maximum patch size
- command allowlist or runner profile allowlist
- timeout limits
- maximum buffered stdout/stderr
- maximum concurrent process count
- optional network policy per process profile

## Suggested Execution Profiles

The contract should allow a small set of named profiles such as:

- `default`
- `test`
- `build`
- `offline`

The exact meaning belongs to runtime policy, not to the model prompt.

## Trace Requirements

Each tool should emit enough trace to support harness assertions.

Suggested trace events:

- `tool_call_requested`
- `tool_call_started`
- `tool_call_completed`
- `process_started`
- `process_output_observed`
- `process_exited`

Minimum structured fields:

- `tool`
- `arguments`
- `status`
- `process_id` for process-related tools
- `exit_code` for completed processes

## Harness Implications

This contract is sufficient to support the minimum M5 suite:

- `m5-read-file-and-answer`
- `m5-edit-file-and-verify`
- `m5-run-process-and-read-output`
- `m5-fix-small-regression`

Those cases should test the contract, not a specific backend such as Docker.

## First-Draft Recommendation

The first M5 delivery should implement only these six core tools and should
prefer:

- one workspace
- text files only
- small repositories
- bounded command profiles
- host-backed execution sandbox

That is enough to make MiniAgentOS a real bounded coding runtime without
overcommitting to a guest-native filesystem or open shell model too early.
