# MiniAgentOS

MiniAgentOS is an agent-first runtime project. This repository starts with a
harness-driven development flow so we can build the runtime against repeatable,
inspectable, QEMU-friendly task loops instead of growing the system without
evaluation hooks.

## Milestones

Current implementation status:

- `M0`: implemented
- `M1`: implemented
- `M2`: implemented
- `M3`: implemented
- `M4`: implemented
- `M5`: implemented in its first bounded form
- `M6`: partially implemented as bounded web search and research, with fixture
  and first live harness slices

Milestone 0 is `Boot to Live Agent`:

- boot into an agent-first prompt
- accept one structured goal
- execute a small skill plan
- perform a real network action
- emit machine-readable trace events
- pass a harness evaluator

The harness in this repository already defines that contract.

Milestone 1 is `Governed Skill Runtime`:

- move beyond one fixed task kind
- add a real skill registry
- enforce per-skill policy and budgets
- call a real model capability
- validate a live OpenAI-backed path with `OPENAI_API_KEY` and `gpt-5.4-mini`
- validate success, refusal, and failure in the harness
- leave the M1 code path in a shape that can be extended without growing one
  giant `runtime/src/main.rs` block

Milestone 2 is `Native Goal Shell`:

- accept constrained natural language directly at `Goal >`
- compile supported goal text locally inside MiniAgentOS
- preserve the JSON task path for harness automation
- stop treating human goal input as a fallback `unknown command` case
- make supported natural-language goals work in manual QEMU interaction

Milestone 3 is `Native Capability Core`:

- make ordinary natural-language interaction the primary `Goal >` path
- add a runtime-owned model-driven agent core
- add a runtime-owned intent compiler that turns goal text into structured
  Intent IR before execution
- keep capability selection bounded, policy-controlled, and trace-visible
- return default task results directly to the shell
- remove any required dependency on a host-side interpretation service such as
  `10.0.2.2:8084`
- require the official live path to connect to OpenAI directly from MiniAgentOS
- allow a host transport proxy only as raw network plumbing, not as a goal or
  model gateway, and use `10.0.2.2:7897` for the current HTTPS proxy setup
- keep host-side glue limited to fixtures, evaluation, and optional secret
  injection support

For M3, "done" now specifically means language/style constraints from natural
language, such as `in Chinese` or `three bullet points`, are preserved as
structured intent state instead of being left implicit in raw prompt text.

Milestone 4 is `Sessioned Tool Loop`:

- replace action-branch-driven execution with a standard `session + model -> tools -> model`
  loop
- make session state a first-class runtime concern
- build model input from explicit layers instead of one undifferentiated prompt
  blob
- make the latest user request a first-class, authoritative section in model
  context
- keep the tool surface small, explicit, and honest
- focus that tool surface on real network/platform capabilities the runtime
  already has
- include X/Twitter read/write capabilities in the initial tool set
- support follow-up turns in the same session
- evaluate loop behavior in the harness, not just one-shot task completion
- use a four-part prompt contract for guest-direct model turns:
  - `Current request`
  - `Latest tool result`
  - `Session state`
  - `Recent conversation`
- reuse the guest-side OpenAI transport across consecutive OpenAI-only turns to
  reduce repeated TCP/SOCKS5/TLS handshakes

Milestone 5 is `Workspace And Execution Substrate`:

- add a bounded workspace abstraction
- add real file inspection and editing tools
- add a real bounded execution primitive
- add observable process output
- start validating true inspect/edit/run/observe coding loops in the harness

M5 is no longer roadmap-only. The repository now has both:

- a fixture-backed `m5` suite for harness-first validation
- a real-runtime `m5live` suite that exercises the bounded coding substrate
  through QEMU, the host bridge, Docker-backed Python execution, and a live
  OpenAI path

The first bounded M5 delivery is intentionally narrow:

- workspace access is bounded and policy-controlled
- execution is Python-first and Docker-backed
- the agent loop supports real inspect/edit/run/observe flows without becoming
  an open shell

Milestone 6 is `Bounded Web Search And Research Runtime`:

- add a bounded `search_web` primitive
- keep search and fetch as separate runtime capabilities
- preserve a truthful known-source set across follow-up turns
- use `BRAVE_API_KEY` for the intended live search path
- validate the first research loop in the harness before the live runtime path
  is complete

The repository now includes:

- a fixture-backed `m6` suite for deterministic search/fetch/follow-up
  validation
- a first `m6live` QEMU-backed search -> fetch -> answer slice

M6 itself is still not a completed runtime milestone.

The formal milestone specs live in:

- `docs/milestones/m0.md`
- `docs/milestones/m1.md`
- `docs/milestones/m2.md`
- `docs/milestones/m3.md`
- `docs/milestones/m4.md`
- `docs/milestones/m5.md`
- `docs/milestones/m6.md`

The fixture agent proves the harness works; it does not count as milestone
completion for the real runtime.

## Repository layout

- `AGENTS.md`: project map and working rules for coding agents
- `docs/milestones/m0.md`: the M0 spec
- `docs/milestones/m1.md`: the M1 spec
- `docs/milestones/m2.md`: the M2 spec
- `docs/milestones/m3.md`: the M3 spec
- `docs/milestones/m4.md`: the M4 spec
- `docs/milestones/m5.md`: the M5 spec
- `docs/milestones/m6.md`: the M6 spec
- `docs/schemas/`: task, trace, and intent artifact schema references
- `harness/`: host-side harness, cases, configs, and fixture services
- `runtime/`: the real MiniOS runtime being adapted to the M0 harness contract
- `tools/m5_host_bridge.py`: host-side M5/M6 bridge for workspace, bounded
  process access, and Brave-backed web search
- `tools/agent_run.py`: one-command manual launcher for the live agent path
- `tools/m5_run.py`: compatibility wrapper for the older launcher name
- `tools/view_llm_log.py`: pretty-printer for `llm_api_log.jsonl`
- `scripts/check.py`: repository validation
- `bin/check`: lightweight repository checks
- `bin/qemu-system-aarch64-local`: wrapper around a locally extracted QEMU bottle
- `bin/run-case`: run one harness case against a configured agent command
- `bin/run-suite`: run a named harness suite against a configured agent command
- `bin/setup-qemu-local`: extract a local QEMU runtime from cached Homebrew bottles
- `bin/setup-toolchain`: install the Rust and QEMU toolchain expected by the harness
- `bin/validate`: run the self-validated harness flow with the fixture agent

## Quick start

Install the local toolchain used by the real runtime and the QEMU harness:

```sh
./bin/setup-toolchain
```

On machines without `/opt/homebrew`, the script installs Homebrew under
`$HOME/homebrew`. In that mode `brew install qemu` may build some dependencies
from source, so the first run can take a while. If that unsupported build
cannot complete, `./bin/setup-toolchain` falls back to a local bottle-based
QEMU runtime under `$HOME/.miniagentos`.

Run the repository checks:

```sh
./bin/check
```

Run the harness end-to-end with the local fixture agent:

```sh
./bin/validate
```

Run a case against a real QEMU command after you have MiniAgentOS booting:

```sh
cd runtime && make build
cp harness/config.example.json harness/config.local.json
# edit harness/config.local.json
./bin/run-case harness/cases/m0-fetch-summarize-post/task.json --config harness/config.local.json
```

The example config assumes the real kernel is launched from `runtime/` and
uses `../bin/qemu-system-aarch64-local` so the harness can run even when a
standard Homebrew `qemu` install is unavailable.

Run the default M1 suite against the real runtime:

```sh
./bin/run-suite --suite m1 --config harness/config.example.json
```

Run the real-model M1 path with OpenAI:

```sh
./bin/run-case harness/cases/m1-allow-fetch-model-post/task.json --config harness/config.openai.json
```

`harness/config.openai.json` uses `OPENAI_API_KEY` and the `gpt-5.4-mini` model
through the current bridge implementation. That config remains useful for M1,
M2, and prototype M3 coverage, but it no longer defines the official M3 live
bar.

In practice, M1 now has two bars:

- stable regression bar: `./bin/run-suite --config harness/config.example.json`
- live model bar: `./bin/run-case harness/cases/m1-allow-fetch-model-post/task.json --config harness/config.openai.json`

Run the M2 natural-language suite against the real runtime:

```sh
./bin/run-suite --suite m2 --config harness/config.example.json
```

Run the initial M4 harness suite against the fixture agent:

```sh
./bin/run-suite --suite m4 --config harness/config.fixture.json
```

Run the fixture-backed M5 suite:

```sh
./bin/run-suite --suite m5 --config harness/config.fixture.json
```

Run the real-runtime M5 suite:

```sh
./bin/run-suite --suite m5live --config harness/config.runtime-m5.json
```

Run the fixture-backed M6 suite:

```sh
./bin/run-suite --suite m6 --config harness/config.fixture.json
```

Run the first live-runtime M6 suite:

```sh
./bin/run-suite --suite m6live --config harness/config.runtime-m6.json
```

Run the live OpenAI-backed M4 summarize flow:

```sh
./bin/run-case harness/cases/m4-loop-summarize-url-openai/task.json --config harness/config.openai.json
```

Run a two-turn live M4 OpenAI follow-up check:

```sh
./bin/run-case harness/cases/m4-loop-openai-followup/task.json --config harness/config.openai.json
```

Run the live OpenAI-backed M2 success path:

```sh
./bin/run-case harness/cases/m2-nl-fetch-model-post/task.json --config harness/config.openai.json
```

The default manual M2 path is now fully local after the fetch:

```text
Fetch http://10.0.2.2:8082/source and summarize it in three bullet points.
```

That path performs one real fetch, summarizes locally inside MiniAgentOS, and
prints the summary directly to UART without any sink or model gateway. The live
OpenAI-backed M2 path still exists for the governed `fetch -> model -> post`
goal family, but the goal compilation stage itself is local to MiniAgentOS.

The harness now also includes an initial M3 case surface under
`harness/cases/m3-*`, plus terminal-result capture for agent flows that return
their final answer directly to UART instead of posting to a sink, and
`intent_ir.json` artifacts for runs that emit structured intent compilation.
Those compiled-intent artifacts now carry explicit user-facing constraints such
as `output_language` and `style`, so cases like `in Chinese` can be evaluated
as intent correctness instead of relying only on prompt inheritance.

Those M3 cases are useful for bridge implementations, but the official M3 bar
now requires the default manual path to work without a host-side interpretation
gateway and requires the live model path to be guest-direct rather than routed
through a host-side OpenAI proxy.

If your network requires a local proxy, MiniAgentOS may still send guest HTTPS
traffic through a plain host-side transport proxy. That is acceptable for M3 as
long as the host is only forwarding bytes and not acting as an interpretation
or OpenAI application gateway. The current runtime expects that transport proxy
at `10.0.2.2:7897`.

For M4-oriented harness work, the runner can now also capture:

- `tool_calls.json`: tool-loop events extracted from trace
- `session_transcript.json`: per-turn transcript and tool/result deltas

Those artifacts are what the first `m4-*` cases use to verify multi-turn loop
behavior and X/Twitter tool calls without depending on the real runtime yet.

The real M4 runtime now builds guest-side model context from four explicit
sections:

- `Current request`
- `Latest tool result`
- `Session state`
- `Recent conversation`

That contract exists so the newest request remains authoritative even after
long prior answers, large tool results, or failed turns.

For manual M4 shell use, the default UI is intentionally quiet. A normal
successful interaction looks like:

```text
Goal > Summarize https://example.com in three bullet points in Chinese.
thinking...
fetching...
summarizing...
页面标题为“Example Domain”，用于文档示例，表示该域名仅供示例使用，无需额外许可。
Goal >
```

M4 now also supports session-style follow-up turns such as:

```text
Goal > Andrej Karpathy's latest opinions in twitter.
...
Goal > Could you summarize that in Chinese?
...
Goal >
```

When you want lower-level visibility, these shell commands are available:

- `status inline`: single-line status updates for manual interactive use
- `status plain`: plain text phase lines (useful for logs and harness runs)
- `status status`: show the current status display mode
- `trace on|off`: enable or disable structured `TRACE {...}` events on UART
- `debug on|off`: enable or disable lower-level transport/debug logging
- `openai-status`: show whether a guest OpenAI key is currently available

For local development, if `OPENAI_API_KEY` is present when you build the guest,
the runtime now embeds that key and auto-loads it at boot. Manual `openai-key`
entry is still available as an override, but it is no longer required for every
session.

For X/Twitter integration:

- `post_tweet` uses OAuth 1.0a user-context credentials
- `search_recent_posts` and `get_user_posts` use `X_BEARER_TOKEN`

The runtime reads those from shell environment variables at build time.

## Manual live usage

The shortest supported manual entrypoint for the live agent path is:

```sh
./tools/agent_run.py
```

By default it:

- uses the current directory as the bounded workspace
- loads `OPENAI_API_KEY` from the current shell or an interactive shell
- starts the host bridge
- launches the runtime with the known-good live M5 environment for this host
- cleans up any prior launcher-owned bridge/runtime process groups
- captures guest trace in the background for log artifacts without printing raw TRACE lines by default
- writes manual runtime artifacts under `output/agent-manual/<timestamp>/`, including:
  - `uart.log`
  - `trace.jsonl`
  - `llm_api_log.jsonl`

The old launcher name `./tools/m5_run.py` still works as a compatibility alias.

On this host, the supported live OpenAI path uses the host SOCKS5 proxy on
`10808`. `tools/agent_run.py` wires that up for you.

Pass `--show-trace` when you want raw guest TRACE lines echoed in the terminal.
Pass `--no-trace-capture` when you want to disable trace capture and live
`llm_api_log.jsonl` updates entirely.

`llm_api_log.jsonl` records each guest-side LLM request/response pair so you can
inspect how the runtime constructed the model context (`Current request`,
`Latest tool result`, `Session state`, `Recent conversation`) and how the model
responded on each turn.

Use the viewer tool when you want a readable rendering instead of raw JSONL:

```sh
./tools/view_llm_log.py --latest
./tools/view_llm_log.py --latest --follow
./tools/view_llm_log.py --follow-latest
./tools/view_llm_log.py --latest --full
./tools/view_llm_log.py --latest --markdown --output /tmp/llm-log.md
```

A typical manual M5 task now looks like:

```text
Goal > Run check.py first. If it fails, fix the minimum code needed in the workspace and run check.py again until it passes.
```

The bounded M5 tool surface currently includes:

- `list_workspace`
- `read_file`
- `write_file`
- `apply_patch`
- `run_process`
- `read_process_output`

## Harness contract

The harness expects the runtime to:

- print a prompt line containing `Goal >`
- accept one JSON task or one supported plain-text goal on stdin
- print trace lines prefixed with `TRACE `
- perform a real HTTP fetch
- optionally POST a result payload to the sink URL when the goal shape requires
  posting

That gives us one stable loop:

`boot -> prompt -> goal -> trace -> local summary or external effect -> evaluation`
