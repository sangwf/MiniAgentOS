# M5 Harness Readiness Checklist

## Purpose

This document captures the decisions that should be fixed before implementing
the first M5 Harness Engineering slice.

The goal is to prevent M5 implementation work from drifting because the harness
environment, execution backend, or artifact contract is still ambiguous.

## Confirmed Decisions

### 1. Primary Language Surface

The first M5 harness surface is Python-first.

This means:

- the first acceptance fixtures should be Python repositories
- the first execution profiles should prioritize Python commands
- the first regression-repair cases should use Python code and `pytest`

### 2. Execution Backend

The first M5 execution backend is Docker.

This means:

- `run_process` should execute inside a bounded Docker sandbox
- harness fixtures should assume Docker is available
- the agent should never directly invoke Docker
- Docker remains an implementation detail of the host-backed runner

## Remaining Harness Decisions

The following items are now fixed as the default first-draft policy.

### 3. Artifact Contract

The first M5 harness preserves at least these artifacts for every case:

- `workspace_before.json`
- `workspace_after.json`
- `process_runs.json`
- `tool_calls.json`
- `session_transcript.json`
- `report.json`

Optional helpful artifacts:

- `file_reads.json`
- `file_writes.json`
- `process_output/<process_id>.stdout`
- `process_output/<process_id>.stderr`

### 4. Workspace Lifecycle

Each case runs against an isolated workspace copy.

Accepted policy:

- copy fixture repo into a fresh workspace directory per run
- mount only that workspace into the Docker sandbox
- destroy the temporary workspace after artifacts are captured

### 5. Execution Policy

The first M5 process allowlist is intentionally narrow.

Accepted Python-first allowlist:

- `python3`
- `pytest`
- `python3 -m pytest`

Accepted denials:

- shell entrypoints such as `sh`, `bash`, `zsh`
- package installation commands such as `pip install`
- arbitrary network tools such as `curl` and `wget`

### 6. Network Policy

Accepted first-draft policy:

- disable network access inside Docker for all M5 process runs

If later M5 cases need network, introduce it as an explicit profile extension,
not as default behavior.

### 7. Budget Defaults

Accepted first-draft budgets:

- max tool calls: 16
- max process runs: 3
- max changed files: 3
- max retained process output: 64 KiB combined
- max file size read/write: 32 KiB

### 8. M4 Coexistence

M5 harness work preserves M4 stability.

Accepted rule:

- M4 suites continue to run and remain valid while M5 is added

M5 should introduce new suites, not reinterpret existing M4 acceptance bars.

## Recommended First Acceptance Slice

The first M5 slice should target exactly these cases:

- `m5-read-file-and-answer`
- `m5-edit-file-and-verify`
- `m5-run-process-and-read-output`
- `m5-fix-small-regression`

All four should use tiny Python fixtures and Docker-backed offline execution.

## Suggested Docker Baseline

The first M5 harness should standardize on one Python image family to keep case
behavior deterministic.

Example baseline:

- Python 3.12 image with `pytest` preinstalled

The exact image tag should be pinned in harness configuration rather than left
implicit in fixture definitions.

## Ready-To-Implement Checklist

M5 harness implementation is ready to begin when all of the following are true:

- Python-first scope is accepted
- Docker backend is accepted
- artifact contract is accepted
- workspace lifecycle is accepted
- execution allowlist is accepted
- offline network policy is accepted
- budget defaults are accepted
- M4 coexistence rule is accepted

## Why This Checklist Matters

If these decisions are left implicit, M5 implementation effort will fragment
across:

- bridge design
- Docker runner details
- fixture design
- evaluator rules
- artifact naming

Fixing the harness environment first keeps M5 aligned with the project’s
existing Harness Engineering discipline.
