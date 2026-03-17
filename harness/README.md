# Harness

This directory contains the host-side harness for MiniAgentOS.

## Components

- `cases/`: task definitions and fixture assets
- `config.example.json`: example config for a real QEMU-backed run
- `config.fixture.json`: config for harness self-validation
- `fixtures/fake_agent.py`: agent fixture used by `./bin/validate`
- `lib/http_fixtures.py`: source, interpretation, model, and result sink servers
- `lib/run_case.py`: case runner
- `lib/evaluator.py`: automated evaluator

## Output artifacts

Each case run produces an output directory with:

- `uart.raw.log`: raw UART byte stream
- `uart.log`: UTF-8 decoded UART output
- `trace.jsonl`: parsed trace events
- `result.json`: sink payload, if any
- `terminal_result.json`: extracted terminal-facing result, if any
- `intent_ir.json`: extracted compiled intent, if the runtime emitted it
- `tool_calls.json`: extracted tool-loop events, if the runtime emitted them
- `session_transcript.json`: per-turn transcript and delta artifacts for multi-turn cases
- `report.json`: evaluator report
- `run.json`: expanded runtime metadata

For M5-style cases, the harness may also preserve:

- `file_reads.json`
- `file_writes.json`
- `file_patches.json`
- `tool_errors.json`
- `process_runs.json`
- `process_output/<process_id>.stdout`
- `process_output/<process_id>.stderr`
- `workspace_before.json`
- `workspace_after.json`

For M3-style cases, the primary success path may return directly to UART instead
of posting to a sink, so the harness now preserves both sink-side and
terminal-side result artifacts. The harness can also validate compiled intent
for M3 by comparing `intent_ir.json` against `expect.expected_intent` in a
case file, which is how language/style constraints such as `output_language=zh`
and `style=bullet` are now verified.

The real MiniAgentOS runtime should eventually be able to plug into this
harness without changing the case format.

## M4 harness surface

The harness now supports an initial M4-style case surface:

- multi-turn `turns[]` case definitions
- per-run `tool_calls.json` extraction from tool-loop trace
- per-run `session_transcript.json` with per-turn outputs and observations
- `expected_tool_calls` and `expected_turn_count` assertions in case `expect`
- mock X fixtures for:
  - `post_tweet`
  - `search_recent_posts`
  - `get_user_posts`

Run the fixture-backed M4 suite with:

```sh
./bin/run-suite --suite m4 --config harness/config.fixture.json
```

## M5 harness surface

The harness now also supports the first bounded coding substrate cases:

- fixture-backed `m5` suite for contract and evaluator validation
- QEMU-backed `m5live` suite for real-runtime validation

Run the fixture-backed M5 suite with:

```sh
./bin/run-suite --suite m5 --config harness/config.fixture.json
```

Run the real-runtime M5 suite with:

```sh
./bin/run-suite --suite m5live --config harness/config.runtime-m5.json
```

The real-runtime M5 config uses:

- QEMU guest runtime
- host M5 bridge
- Docker-backed bounded Python execution
- a live OpenAI path

## Toolchain paths

`config.example.json` includes `path_prefixes` so the harness can find
`qemu-system-aarch64` and `cargo` even when they live under a user-local
Homebrew or Rust install. The launcher prepends those paths before starting
the configured agent command. The default real-runtime config calls the local
wrapper in `../bin/qemu-system-aarch64-local`, which uses a bottle-extracted
QEMU fallback when a standard Homebrew install is unavailable.

The real-runtime configs now default fixture ports to `0`, letting the OS pick
free ports per run. This keeps suites stable even when multiple QEMU runs or
watchers are active on the same machine.
