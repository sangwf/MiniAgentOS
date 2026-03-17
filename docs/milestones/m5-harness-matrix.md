# M5 Harness Case Matrix Draft

## Purpose

This document defines the first Harness Engineering case surface for M5.

The goal is to validate that MiniAgentOS can complete a real bounded coding
loop:

`inspect -> edit -> execute -> observe -> iterate`

against a real workspace using the M5 tool contract.

## Acceptance Strategy

M5 should not be accepted by demoing one impressive ad hoc run.

It should be accepted by a compact but rigorous harness matrix that proves:

- workspace inspection is real
- file editing is real
- execution is real
- process observation is real
- the loop can iterate based on failure output

## Case Groups

### Group A: Workspace Inspection

These cases prove the agent can reason about the bounded workspace.

#### `m5-read-file-and-answer`

Goal:

- agent lists the workspace
- agent reads the relevant file
- agent answers a question from file contents without editing

Expected behaviors:

- at least one `list_workspace`
- at least one `read_file`
- no file modifications
- no process execution required

Harness assertions:

- terminal result contains the expected answer
- tool calls include `list_workspace` and `read_file`
- no `write_file` / `apply_patch`
- no `run_process`

Suggested fixture:

- small repo with `README.md` and one source file
- user asks: "Which function returns the greeting string?"

#### `m5-deny-workspace-escape`

Goal:

- agent attempts to read a path outside the bounded workspace
- runtime refuses the request in a structured way
- agent reports the refusal reason

Expected behaviors:

- `read_file`
- no workspace modification
- no process execution

Harness assertions:

- tool call completes with a denied/error result
- refusal reason matches the workspace path policy
- `tool_errors.json` records the invalid path

Suggested fixture:

- minimal workspace with one harmless file
- user asks: "Read ../outside.txt and tell me what it says."

### Group B: Direct File Modification

These cases prove the agent can make a real workspace edit.

#### `m5-edit-file-and-verify`

Goal:

- agent reads one file
- agent makes one bounded edit
- agent reads the file again or returns a summary of the edit

Expected behaviors:

- `read_file`
- `write_file` or `apply_patch`
- no execution required

Harness assertions:

- final file content matches expected content
- tool calls include one edit tool
- changed files stay within allowed scope

Suggested fixture:

- one Python file contains `print("helo")`
- user asks: "Fix the typo in the output string."

#### `m5-apply-patch-and-verify`

Goal:

- agent reads one file
- agent applies one bounded patch
- agent returns a summary of the edit

Expected behaviors:

- `read_file`
- `apply_patch`
- no execution required

Harness assertions:

- final file content matches expected content
- tool calls include `apply_patch`
- patch stays within the allowed workspace scope

Suggested fixture:

- one Python file contains `print("helo")`
- user asks: "Apply a patch to fix the typo in the output string."

### Group C: Bounded Execution

These cases prove the runtime can launch a real execution task and observe it.

#### `m5-run-process-and-read-output`

Goal:

- agent runs one bounded command
- agent reads output
- agent reports what happened

Expected behaviors:

- `run_process`
- `read_process_output`
- no file modification required

Harness assertions:

- one process is started
- output is captured
- exit status is captured
- terminal answer reflects real process output

Suggested fixture:

- workspace includes `main.py` that prints `42`
- allowlisted process profile runs `python3 main.py`

#### `m5-deny-inline-python`

Goal:

- agent requests a disallowed execution shape
- runtime denies the request in a structured way
- agent reports the denial reason

Expected behaviors:

- `run_process`
- no actual process is started
- terminal result is a refusal, not a crash

Harness assertions:

- tool call completes with a denied/error result
- refusal reason matches the bounded runner policy
- `process_runs.json` remains empty

Suggested fixture:

- workspace can be minimal
- user asks: "Run inline python with -c and tell me whether it is allowed."

### Group D: Single-Step Repair

These cases prove the agent can use execution output to perform one repair.

#### `m5-fix-small-regression`

Goal:

- agent inspects a small repo
- agent runs one bounded verification command
- command fails
- agent edits code
- agent reruns verification
- command passes

Expected behaviors:

- `list_workspace`
- `read_file`
- `run_process`
- `read_process_output`
- `write_file` or `apply_patch`
- second `run_process`

Harness assertions:

- first run exits non-zero
- second run exits zero
- final file contents match expected repair intent
- total tool call count stays within budget

Suggested fixture:

- tiny Python repo with one failing test
- bug is localized to one function

### Group E: Multi-File Repair

These are stretch cases for later M5, not required for the first acceptance
bar.

#### `m5-fix-cross-file-regression`

Goal:

- agent must inspect multiple files
- agent changes more than one file
- reruns bounded verification

Expected behaviors:

- multi-file reads
- one bounded patch or multiple writes
- rerun and pass

Harness assertions:

- changed file set is bounded and expected
- process output proves first failure and final success

## Recommended First Acceptance Bar

The first M5 implementation should require only these cases:

- `m5-read-file-and-answer`
- `m5-deny-workspace-escape`
- `m5-edit-file-and-verify`
- `m5-apply-patch-and-verify`
- `m5-run-process-and-read-output`
- `m5-deny-inline-python`
- `m5-fix-small-regression`

That bar is enough to prove MiniAgentOS has become a real bounded coding
runtime.

## Real-Runtime Validation Slice

Once the fixture bar is green, the next acceptance layer should include at
least one QEMU-backed live case that exercises the actual guest/runtime path.

### `m5live-run-process-and-read-output`

Goal:

- boot the real MiniAgentOS runtime in QEMU
- send one live OpenAI-backed user request
- have the real guest loop call `run_process`
- have the real guest loop call `read_process_output`
- return a final answer from the observed stdout

Expected behaviors:

- the guest uses the bounded M5 bridge tool surface, not shell commands
- the host bridge emits `process_runs.json` and `process_output/*`
- the final answer reflects real observed output

Harness assertions:

- `tool_calls.json` includes `run_process` and `read_process_output`
- `process_runs.json` records one bounded Python run
- terminal result contains the expected stdout text
- the case is driven by the real runtime and real model, not the fixture agent

### `m5live-fix-small-regression`

Goal:

- boot the real MiniAgentOS runtime in QEMU
- send one live OpenAI-backed repair request
- have the real guest loop run `check.py`
- observe the failing result
- inspect and patch the minimum code needed
- rerun `check.py` until success is observed
- return a final answer that states the verification now passes

Expected behaviors:

- the guest uses the bounded M5 bridge tool surface, not shell commands
- the model does not stop after editing alone
- the host bridge emits both bounded process-run artifacts and patch artifacts
- the final answer is grounded in a real passing rerun

Harness assertions:

- `tool_calls.json` includes an ordered repair subset:
  `run_process -> read_process_output -> ... -> apply_patch -> run_process -> read_process_output`
- `process_runs.json` proves at least one failing run followed by a passing run
- `workspace_after.json` shows the bounded repair landed in the expected file
- terminal result includes the required verification phrase

Together, `m5live-run-process-and-read-output` and
`m5live-fix-small-regression` satisfy the real-runtime M5 acceptance bar.

## Suggested Artifact Additions

The first M5 harness should preserve at least:

- `workspace_before.json`
- `workspace_after.json`
- `process_runs.json`
- `tool_calls.json`
- `session_transcript.json`
- `report.json`

Optional helpful additions:

- `file_reads.json`
- `file_writes.json`
- `file_patches.json`
- `tool_errors.json`
- `process_output/<process_id>.stdout`
- `process_output/<process_id>.stderr`

These artifacts are part of the recommended first M5 acceptance contract and
should be treated as default, not optional design ideas.

## Suggested Process Run Record

Each process run artifact should capture:

```json
{
  "process_id": "proc_0001",
  "argv": ["python3", "main.py"],
  "cwd": "",
  "profile": "default",
  "status": "exited",
  "exit_code": 0,
  "stdout_bytes": 3,
  "stderr_bytes": 0,
  "timed_out": false
}
```

## Budget Guidance

The first M5 harness should keep the task surface intentionally small.

Suggested default limits:

- max changed files: 3
- max process runs: 3
- max tool calls: 16
- max workspace file size: 32 KiB per file
- max process output retained: 64 KiB combined

These values should be used as the first-draft default harness budgets unless a
specific case explicitly needs a narrower cap.

## Fixture Guidance

The first M5 fixtures should be:

- tiny repositories
- single-language or near-single-language
- dependency-stable
- executable with preinstalled host tooling
- small enough that the model can reason over the relevant files without long
  retrieval loops

Good first targets:

- Python toy repo with `pytest`

The first accepted M5 fixture language is Python. Additional language surfaces
such as Node or Rust should be deferred until the Python-first slice is stable.

## What To Avoid In First M5 Cases

- large repositories
- flaky networked builds
- dependency installation during the run
- long-running servers
- multiple unrelated failures in one fixture
- acceptance criteria that depend on subjective code quality judgment

## Definition Of Harness Success

The M5 harness is doing its job when a failing case can be explained in terms
of:

- wrong workspace inspection
- wrong edit
- wrong execution request
- wrong interpretation of process output
- budget or policy violation

If failures are instead dominated by vague prompt-quality arguments, the harness
surface is still underspecified.
