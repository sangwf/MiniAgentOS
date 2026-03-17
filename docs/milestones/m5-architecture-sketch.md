# M5 Host-Backed Architecture Sketch

## Purpose

This document sketches the first implementable M5 architecture.

The goal is to realize the M5 bounded coding substrate using:

- a runtime-owned tool contract inside the MiniAgentOS guest
- a host-backed workspace manager
- a host-backed bounded execution runner

This architecture is intentionally optimized for Harness Engineering rather
than for early guest-native operating-system completeness.

## High-Level Model

The first M5 implementation is split into two planes:

- guest control plane
- host substrate plane

### Guest Control Plane

Lives inside MiniAgentOS and owns:

- session loop
- model/tool orchestration
- tool contract
- policy decisions that are visible to the agent
- trace emission

### Host Substrate Plane

Lives outside the guest and owns:

- workspace storage
- file operation realization
- process sandbox execution
- process output capture

The guest should see a truthful tool surface, but it does not need to know
whether the substrate is implemented locally or via a host bridge.

## Main Components

### 1. Guest M5 Tool Layer

This extends the current M4 tool loop with:

- `list_workspace`
- `read_file`
- `write_file`
- `apply_patch`
- `run_process`
- `read_process_output`

Responsibilities:

- validate tool-call shape
- enforce guest-visible policy
- send structured requests to the bridge
- emit tool-call trace events
- feed tool results back into the agent loop

### 2. Guest/Host Bridge

This is the boundary transport between MiniAgentOS and the host-backed M5
substrate.

Responsibilities:

- receive structured tool requests from the guest
- forward them to host-side workspace/runner services
- return structured tool results
- map bridge failures into runtime-visible error results

The bridge is not a planner and not a hidden agent. It is a transport and
realization layer only.

### 3. Workspace Manager

Host-side service responsible for bounded workspace operations.

Responsibilities:

- create or mount the workspace root for a run
- enforce path boundaries
- perform list/read/write/patch operations
- snapshot workspace state for harness artifacts

Suggested responsibilities by tool:

- `list_workspace` -> directory walk with depth and hidden-file rules
- `read_file` -> bounded text read
- `write_file` -> bounded full-file replace/create
- `apply_patch` -> bounded patch application with file-scope validation

### 4. Sandbox Runner

Host-side bounded execution backend.

Responsibilities:

- validate execution request against runner policy
- launch a process in a bounded sandbox
- capture stdout/stderr/exit status
- expose process state through process handles

This runner may be implemented using Docker in the first M5 version.

### 5. Harness Adapter

Host-side harness integration layer.

Responsibilities:

- prepare workspace fixtures
- configure the runtime/bridge for a case
- collect workspace and process artifacts
- evaluate outcomes using expected traces, edits, and process results

## Suggested Data Flow

### File Read Flow

1. user submits a coding request
2. guest model chooses `read_file`
3. guest emits `tool_call_requested`
4. guest sends structured request to bridge
5. bridge forwards request to workspace manager
6. workspace manager reads the real file under the workspace root
7. result returns through bridge to guest
8. guest emits `tool_call_completed`
9. model receives file contents in the next loop step

### Process Run Flow

1. guest model chooses `run_process`
2. guest emits `tool_call_requested`
3. guest sends execution request to bridge
4. bridge forwards request to sandbox runner
5. sandbox runner validates the request against runner policy
6. sandbox runner starts a real process in the sandbox
7. runner returns `process_id`
8. guest emits `tool_call_completed`
9. model later calls `read_process_output`
10. runner returns stdout/stderr/status/exit code
11. guest feeds that result back into the next model step

## Suggested Runtime Objects

### Workspace

Each run should have one workspace object:

```json
{
  "workspace_id": "ws_0001",
  "root": "/host/workspaces/run-0001",
  "limits": {
    "max_file_size": 32768,
    "max_patch_size": 16384
  }
}
```

### Process Handle

Each process should be represented by a bounded handle:

```json
{
  "process_id": "proc_0001",
  "workspace_id": "ws_0001",
  "profile": "test",
  "argv": ["pytest", "-q"],
  "cwd": "",
  "status": "running"
}
```

## Docker-Backed Runner Sketch

If Docker is used as the first backend, the shape should be:

- one short-lived container per process run
- one mounted workspace root at `/workspace`
- one bounded working directory under `/workspace`
- no privileged mode
- no host socket access
- optional network disable by profile

Conceptually:

```text
MiniAgentOS guest
  -> run_process(argv=["pytest","-q"], cwd="", profile="test")
  -> bridge
  -> sandbox runner
  -> docker run --rm -v <workspace>:/workspace -w /workspace <image> pytest -q
  -> stdout/stderr/exit_code
  -> read_process_output
```

The agent should not receive `docker` as a tool or as a command target.

## Suggested Bridge API Shape

The bridge can be modeled as a small internal RPC surface.

Examples:

### List workspace request

```json
{
  "op": "list_workspace",
  "workspace_id": "ws_0001",
  "path": "",
  "depth": 2,
  "include_hidden": false
}
```

### Run process request

```json
{
  "op": "run_process",
  "workspace_id": "ws_0001",
  "argv": ["python3", "main.py"],
  "cwd": "",
  "profile": "default",
  "timeout_sec": 20
}
```

### Read process output request

```json
{
  "op": "read_process_output",
  "process_id": "proc_0001",
  "offset": 0,
  "limit": 8192
}
```

## Failure Model

The architecture should distinguish at least four failure classes:

- tool argument invalid
- policy denied
- bridge/backend failure
- process exited with non-zero status

These should not collapse into one generic error string because harness
evaluation and agent behavior need to distinguish them.

## Recommended First-Draft Storage Model

The workspace manager should start simple:

- one host directory per harness run
- fixture copied into that directory before execution
- no persistent cross-run mutation
- workspace snapshot captured before and after the run

This keeps M5 deterministic and makes harness evaluation straightforward.

## Recommended Artifact Flow

For each M5 case, the harness should preserve:

- the initial workspace snapshot
- the final workspace snapshot
- process run records
- process output artifacts
- tool call trace
- session transcript

This keeps the acceptance bar centered on observed substrate behavior rather
than prompt mysticism.

## Why This Architecture Fits Current MiniAgentOS

This approach matches the current project shape because MiniAgentOS already has:

- a strong agent control plane
- a traceable loop
- bounded tools
- harness-first development habits

It does not yet have:

- a guest-native filesystem
- a guest-native process model
- a guest-native shell/runtime stack for multiple languages

So the architecture should preserve what already works and externalize the
first coding substrate rather than forcing premature guest-native operating
system breadth.
