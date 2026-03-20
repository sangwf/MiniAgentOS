# Task Plan

## Goal

Define Milestone 6 around bounded web search and research, using `BRAVE_API_KEY`
from the user's shell environment as the live search backend secret.

## Phases

| Phase | Status | Notes |
| --- | --- | --- |
| Capture current state | completed | Read M5 doc, confirm no M6 doc exists, initialize planning files |
| Define M6 scope | completed | Set objective, non-goals, tool surface, and live backend assumptions around Brave |
| Summarize recommendation | completed | Wrote formal M6 milestone doc and synced repo navigation |
| Implement first M6 harness slice | completed | Added deterministic search substrate, artifacts, cases, suite wiring, and docs |
| Implement first live M6 slice | completed | Added Brave-backed search bridge/runtime wiring, `m6live` case, and reconciled live M5/M6 routing and parse robustness |
| Harden guest-native external fetch | completed | Fixed external-site TLS/body handling enough for Guardian-style pages to fetch and summarize without invalid-record or false redirect loops |
| Tighten live research loop behavior | completed | Stopped redundant second `search_web` after a successful search plus supporting fetch, restoring `m6live` to a single-search pass |

## Constraints

- Keep M6 additive on top of M4/M5, not a runtime replacement.
- Search should be bounded and policy-controlled.
- Live backend should use `BRAVE_API_KEY`.
- Do not start implementation in this turn unless the user asks.

## Open Questions

- Whether M6 should include only search + fetch, or also explicit research memory
- Whether to expose one search tool or a small result-management tool family

## Outcome

- Added `docs/milestones/m6.md`
- Added `docs/milestones/m6-tool-contract.md`
- Added `docs/milestones/m6-harness-matrix.md`
- Added `docs/milestones/m6-artifact-contract.md`
- Added `docs/milestones/m6-search-backend.md`
- Added `docs/milestones/m6-harness-readiness-checklist.md`
- Updated `README.md` milestone status and milestone document lists
- Updated `AGENTS.md` project map and added an `M6 Direction` section
- Added `harness/config.runtime-m6.json`
- Added `harness/cases/m6live-search-and-answer/`

## Current Assessment

- M6 is now definition-complete at the document layer.
- The first fixture-backed M6 harness slice is now implemented and passing.
- The first live M6 search slice is now implemented and passing.
- The remaining work is broader live research coverage, not initial backend/runtime integration.
- The original Guardian/manual M6 blocker is now resolved far enough for live
  manual use: the external fetch path no longer dies as `invalid_record`, no
  longer falsely redirects to `.onion`, and can summarize the Guardian repro
  page successfully.
- The earlier `m6live` harness flake from a redundant second `search_web` is
  now resolved in the runtime prompt contract; the live case again passes with a
  single search followed by one supporting fetch.
- Large host-bridge POST requests on the plain HTTP path are now segmented
  correctly instead of being sent as one oversize TCP payload, so manual coding
  prompts no longer fail as `virtio-net tcp payload too large`.

## 2026-03-20 Follow-Up Goal

Add a host-visible log artifact that records each guest-side LLM API request and
response pair so manual and harness runs can be inspected for context and memory
behavior.

### 2026-03-20 Phases

| Phase | Status | Notes |
| --- | --- | --- |
| Add guest trace snapshots | completed | Added request/response snapshot trace events for goal interpretation, session model turns, and summary-model turns |
| Extract host-side LLM log artifact | completed | Added `harness/lib/llm_log.py` and wired `llm_api_log.jsonl` into harness output |
| Add manual-run logging | completed | `tools/m5_run.py` now captures UART/TRACE through a PTY, auto-enables `trace on`, and writes `uart.log`, `trace.jsonl`, and `llm_api_log.jsonl` |
| Revalidate live shared paths | completed | `m6live` passed directly; `m5live` needed only a case expectation refresh to allow an initial `list_workspace` before the first run |
| Rename the manual launcher | completed | Added `tools/agent_run.py` as the canonical entrypoint, kept `tools/m5_run.py` as a compatibility alias, and renamed manual output roots to `output/agent-manual/` |
| Add a readable LLM log viewer | completed | Added `tools/view_llm_log.py` with latest-file lookup, turn filtering, plain text rendering, and Markdown export |
| Hide trace bootstrap from the terminal | completed | Kept background trace capture for live log viewing, but changed the launcher bootstrap so `trace on` and the bootstrap TRACE burst no longer leak into the interactive terminal |
| Add context-engineering viewer modes | completed | Extended `tools/view_llm_log.py` with raw request/response, budget estimates, focus filtering, and per-turn diff views |
| Fix plain-HTTP oversized request sends | completed | Added segmented `FETCH_HTTP` sending with per-request retry so large host-bridge POSTs no longer exceed the virtio TCP payload budget |

## 2026-03-20 Follow-Up Goal (M7)

Define Milestone 7 around durable memory and explicit context management so
MiniAgentOS can sustain longer coding and research loops without relying on
opaque prompt growth.

### 2026-03-20 M7 Phases

| Phase | Status | Notes |
| --- | --- | --- |
| Confirm the next milestone direction | completed | Reused the earlier conclusion that M7 should focus on memory/context rather than another isolated capability |
| Write the formal M7 milestone doc | completed | Added `docs/milestones/m7.md` around durable memory, truthful compaction, inspectability, and resume |
| Sync repository navigation | completed | Updated `README.md` and `AGENTS.md` to list `m7.md` and summarize M7 direction |
| Write the M7 tool contract | completed | Added `docs/milestones/m7-tool-contract.md` with inspection-first memory surfaces, prompt-layer concepts, and bounded checkpoint/compaction semantics |
| Write the M7 harness matrix | completed | Added `docs/milestones/m7-harness-matrix.md` with fixture/live case groups for memory inspection, compaction, follow-up reuse, and resume |
| Write the M7 artifact contract | completed | Added `docs/milestones/m7-artifact-contract.md` to define stable memory/context/checkpoint artifacts for harness evaluation |
| Write the M7 memory backend draft | completed | Added `docs/milestones/m7-memory-backend.md` to define guest RAM memory, context assembly, compaction, and host persistence roles |
| Implement the first M7 fixture harness slice | completed | Added `harness/lib/m7_substrate.py`, wired M7 artifacts into `run_case.py` and `evaluator.py`, extended `fake_agent.py`, and landed a passing `m7` fixture suite |
| Implement the first in-guest M7 runtime slice | completed | Added `runtime/src/agent/memory.rs`, wired memory lifecycle into session/history updates, extended prompt assembly with bounded memory sections, and exposed `memory-status`, `memory-list`, `memory-read` plus matching M4 tools |
| Implement the first live M7 slice | completed | Added trace-backed memory/context artifact synthesis, `harness/config.runtime-m7.json`, `m7live-memory-inspection`, and passed `./bin/run-suite --suite m7live --config harness/config.runtime-m7.json` |
| Implement guest-side truthful compaction first slice | completed | Added automatic bounded source/workspace/execution/conversation compaction in `runtime/src/agent/memory.rs`, emitted `memory_compacted` trace, added `m7live-truthful-compaction`, and passed the full `m7live` suite |

## M7 Current Assessment

- M7 is no longer planned-only.
- The first fixture-backed M7 harness slice is implemented and passing:
  - `./bin/run-suite --suite m7 --config harness/config.fixture.json`
- The harness now emits and evaluates:
  - `memory_snapshot.json`
  - `memory_events.json`
  - `context_snapshot.json`
  - `context_budget.json`
  - `checkpoint_snapshot.json`
- Existing fixture baselines still hold:
  - `m6` passes
  - `m5` passes
  - `m4` remains at the two known failures:
    - `m4-loop-context-bleed-repro`
    - `m4-loop-openai-followup`
- The first real runtime M7 slice now exists, but it is still inspection-first:
  - guest memory lives in RAM only
  - one retained slot per major memory class
  - shell inspection commands work
  - prompt assembly now includes `Working memory`, `Known sources`, and `Workspace memory`
- The first live M7 harness slice is now implemented and passing:
  - `./bin/run-suite --suite m7live --config harness/config.runtime-m7.json`
- The guest-side live runtime now has a first truthful-compaction slice for
  large retained source/workspace/execution/conversation results.
- M7 is still partial overall because checkpoint/resume are still fixture-only,
  and the guest-side compaction policy is still a bounded first slice rather
  than the final durable memory runtime.
