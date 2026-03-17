# M5 Runner Policy Draft

## Purpose

This document defines the first execution policy for the M5 `run_process`
tool.

The goal is to make M5 execution:

- real
- bounded
- repeatable
- harness-friendly

without requiring an unrestricted shell.

## Policy Model

`run_process` should not expose arbitrary shell syntax in the first M5
implementation.

Instead, it should accept:

- structured `argv`
- a workspace-relative `cwd`
- a named execution `profile`
- a bounded `timeout_sec`

The runtime then validates the request against a runner policy.

## First-Draft Principle

The first M5 runner policy should optimize for:

- tiny repositories
- deterministic local execution
- preinstalled toolchains only
- no dependency installation during the run
- straightforward Harness Engineering assertions

This means the first draft should prefer an allowlist model over an
open-command model.

## Recommended Profiles

The first M5 implementation should support these execution profiles:

- `default`
- `test`
- `build`
- `offline`

Profiles are runtime policy categories, not user-facing shell presets.

## Profile Semantics

### `default`

For direct program execution or simple script runs.

Allowed examples:

- `python3 main.py`

Restrictions:

- network disabled by default
- no package manager commands
- bounded stdout/stderr capture

### `test`

For verification-oriented commands.

Allowed examples:

- `pytest -q`
- `python3 -m pytest -q`

Restrictions:

- network disabled
- no dependency installation
- timeout may be slightly larger than `default`

### `build`

For build-only commands where build success matters but long compilation loops
must still stay bounded.

Allowed examples:

- `python3 -m py_compile main.py`

Restrictions:

- network disabled
- bounded wall-clock timeout
- bounded output

### `offline`

For explicitly networkless runs where the harness wants maximum determinism.

Allowed examples:

- `python3 script.py`

Restrictions:

- network disabled
- same command allowlist style as other profiles
- intended for fixtures that must be reproducible without external services

## First-Draft Command Allowlist

The first M5 implementation should keep the allowlist intentionally small.

Recommended language/tooling surface:

- Python:
  - `python3`
  - `pytest`

Additional language/tooling surfaces should be added only after the Python-first
slice is stable in harness.

## Denied Commands In First Draft

The runner should reject at least:

- `sh`
- `bash`
- `zsh`
- `fish`
- `curl`
- `wget`
- `pip`
- `pip3`
- `npm install`
- `pnpm`
- `yarn`
- `git`
- `docker`
- any command not on the allowlist

The agent should never directly invoke the sandbox backend.

## Argument Policy

The policy should validate not only the program name but also the argument
shape.

Examples:

- allowed:
  - `["cargo", "test", "--quiet"]`
  - `["python3", "main.py"]`
  - `["pytest", "-q"]`
- rejected:
  - `["cargo", "install", "ripgrep"]`
  - `["python3", "-c", "import os; ..."]`
  - `["bash", "-lc", "pytest -q"]`

The first draft should reject inline script execution forms such as `python3 -c`
because they blur the boundary between file execution and unrestricted command
injection.

## Working Directory Policy

`cwd` must:

- be relative to the workspace root
- resolve inside the workspace
- exist as a directory before execution

Empty `cwd` means workspace root.

## Timeout Policy

Recommended first-draft defaults:

- `default`: 20 seconds
- `test`: 30 seconds
- `build`: 45 seconds
- `offline`: 20 seconds

Recommended hard cap:

- 60 seconds maximum regardless of request

## Output Policy

Recommended output limits:

- stdout retained: 48 KiB
- stderr retained: 16 KiB
- combined retained output: 64 KiB

If output exceeds the retained limit:

- preserve a prefix or bounded rolling window
- mark the result as `truncated: true`

## Concurrency Policy

The first M5 implementation should keep process handling simple.

Recommended rules:

- one active process per run by default
- optional later increase to two concurrent processes
- no background daemon management in the first draft

## Network Policy

The safest first-draft choice is:

- network disabled for all M5 process profiles by default

If later cases need network, that should be introduced as an explicit profile
addition rather than quietly enabled under `default`.

## Sandbox Backend Policy

The runner backend may be host-backed and may use a container sandbox such as
Docker.

The backend should enforce:

- mounted workspace root only
- read/write access only to the mounted workspace
- no host Docker socket access
- no privileged mode
- no background service persistence across runs
- profile-specific network policy

## Trace Requirements

Every execution request should emit trace with enough detail for harness
evaluation.

Suggested trace fields:

- `tool: "run_process"`
- `arguments.argv`
- `arguments.cwd`
- `arguments.profile`
- `status`
- `process_id`
- `exit_code`
- `timed_out`

## First-Draft Recommendation

For the first M5 implementation, the runner policy should be:

- allowlist-based
- profile-based
- offline by default
- single-process
- no package installation
- no shell language
- Python-first

That is narrow enough to stay honest and stable, but still wide enough to
support the first M5 coding loop cases.
