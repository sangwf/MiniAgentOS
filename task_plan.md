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
