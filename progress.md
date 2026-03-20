# Progress

## 2026-03-17

- Read the `planning-with-files` skill instructions.
- Confirmed planning files were absent and re-created them.
- Read the current M5 milestone definition to anchor the next milestone design.
- Confirmed there is no existing M6 milestone text in the repository.
- Checked current live search backend options and narrowed the recommended M6 live provider to Brave Search via `BRAVE_API_KEY`.
- Added `docs/milestones/m6.md` with M6 objective, non-goals, tool surface, backend model, compatibility requirements, and harness bar.
- Added `docs/milestones/m6-tool-contract.md` with the first `search_web` contract, source-memory model, artifact expectations, and `fetch_url` interaction rules.
- Added `docs/milestones/m6-harness-matrix.md` with fixture/live split, search/fetch separation cases, follow-up cases, and refusal cases.
- Added `docs/milestones/m6-artifact-contract.md` to define the first stable M6 artifact set and the minimum fields evaluators can rely on.
- Added `docs/milestones/m6-search-backend.md` to define the fixture backend, live Brave adapter, bridge API shape, and policy boundaries.
- Added `docs/milestones/m6-harness-readiness-checklist.md` to lock the remaining pre-implementation harness decisions.
- Updated `README.md` and `AGENTS.md` so repository navigation reflects the new M6 milestone document.
- Ran `./bin/check` after the doc updates; it passed.
- Implemented `harness/lib/m6_substrate.py` to back deterministic search results, fetched-source artifacts, and source-memory artifacts.
- Extended `harness/lib/http_fixtures.py` so one source fixture can serve multiple deterministic pages for M6 cases.
- Extended `harness/fixtures/fake_agent.py` with M6 search, fetch, compare, and follow-up behavior.
- Extended `harness/lib/run_case.py`, `harness/lib/evaluator.py`, and `harness/lib/run_suite.py` for M6 artifact loading, assertions, and suite selection.
- Added six fixture-backed M6 cases under `harness/cases/m6-*`.
- Updated `README.md`, `AGENTS.md`, and `harness/README.md` to document the new fixture-backed M6 surface.
- Validation run results:
  - `./bin/check` passed
  - `python3 -m py_compile ...` passed for the changed Python modules
  - `./bin/run-suite --suite m6 --config harness/config.fixture.json` passed
  - `./bin/run-suite --suite m5 --config harness/config.fixture.json` passed
  - `./bin/run-suite --suite m4 --config harness/config.fixture.json` still failed only the two pre-existing known cases
- Attempted to remove generated `__pycache__` directories, but the command was blocked by policy in this environment.
- Extended `tools/m5_host_bridge.py` into the first shared M5/M6 host bridge:
  - added Brave-backed `/search/web`
  - added `search_results.json` emission
  - added shell-environment lookup for `BRAVE_API_KEY`
  - added HTML fallback parsing for Brave search result pages when the API curl path is unavailable
- Extended `runtime/src/main.rs` with manual `m6-search` support and `runtime/src/agent/loop.rs` with the real `search_web` tool.
- Added `harness/config.runtime-m6.json` and `harness/cases/m6live-search-and-answer/` for the first real-runtime M6 validation path.
- Implemented tool-sensitive post-bridge OpenAI routing:
  - `search_web` keeps post-tool model turns on guest-native OpenAI transport
  - coding-oriented bridge tools switch post-tool model turns back to the host OpenAI relay
- Added bounded parse-error retry in the M4 loop so occasional non-JSON planning asides from the model do not immediately fail live coding cases.
- Validation run results after live M6 + loop hardening:
  - `./bin/run-case harness/cases/m5live-fix-small-regression/task.json --config harness/config.runtime-m5.json --output output/m5live-regression-retest` passed
  - `./bin/run-case harness/cases/m6live-search-and-answer/task.json --config harness/config.runtime-m6.json --output output/m6live-retest` passed
  - `./bin/run-suite --suite m5live --config harness/config.runtime-m5.json` passed
  - `./bin/run-suite --suite m6live --config harness/config.runtime-m6.json` passed
  - `./bin/run-suite --suite m6 --config harness/config.fixture.json` passed
  - `./bin/run-suite --suite m5 --config harness/config.fixture.json` passed
  - `./bin/run-suite --suite m4 --config harness/config.fixture.json` still failed only the two known M4 cases
  - `./bin/check` passed
- Updated `docs/milestones/m6.md`, `README.md`, `harness/README.md`, and `AGENTS.md` so the repository reflects the first live M6 slice instead of describing M6 as planned-only.

## 2026-03-18 TLS Debug Follow-Up

- Reproduced the external-site fetch failure in a manual QEMU session with
  `trace on` using a Guardian news URL.
- Confirmed the guest-native HTTPS failure code is `-0x7200`
  (`MBEDTLS_ERR_SSL_INVALID_RECORD`) during `https_read`.
- Verified from trace that the failure happens after `tls_handshake` completes
  and before any HTTP response bytes are successfully parsed.
- Hardened the TLS receive path in `runtime/src/tls.rs`:
  - enlarged `TLS_RX_BUF`
  - enlarged pending-slot count and per-slot size
  - retained the longer payload when an exact-seq future retransmit arrives
  - added counters for RX compaction, RX overflow reset, pending store/replace,
    duplicate drop, oversize drop, no-slot drop, and replay activity
  - added `invalid_record` to the human-readable TLS error labels
- Extended `tls-status` in `runtime/src/main.rs` so the new counters are visible
  from the guest shell after a failing fetch.
- Rebuilt successfully with `cd runtime && make build`.
- Re-ran the manual Guardian repro on the new binary. The fetch no longer failed
  in exactly the same place; instead it later retried and ended as
  `tcp connect timed out`. This means the original invalid-record path was
  disturbed by the hardening, but the arbitrary-site HTTPS fetch problem is not
  fully resolved yet.

## 2026-03-19 External Fetch Resolution

- Re-read the current planning files and continued the external-site HTTPS debug
  line instead of changing milestone scope again.
- Confirmed on a fresh manual QEMU run that the latest receive-path hardening
  had already changed the Guardian repro from `tls read failed` /
  `invalid_record` into `response body truncated`.
- Measured the real Guardian page size on the host with `Accept-Encoding:
  identity` and confirmed it is about `315640` bytes, far above the old
  `65536`-byte agent response capture limit.
- Increased `AGENT_RESPONSE_BODY` to `524288` bytes in
  `runtime/src/main.rs`.
- Updated the M4/M5/M6 loop so a successful `fetch_url` with a non-empty but
  truncated body can still continue instead of being treated as an immediate
  tool failure.
- Extended the `fetch_result_snapshot` trace with `body_truncated` and made the
  fetch preview explicitly mark truncated content.
- Reproduced a successful raw shell fetch of the Guardian page and discovered
  the next hidden bug: after the successful `200 OK`, the runtime was
  mis-parsing the `onion-location:` header as a true `Location:` redirect and
  launching a second doomed fetch to a `.onion` URL.
- Tightened `find_location()` in `runtime/src/main.rs` so only a real
  line-start `Location:` header is treated as a redirect target.
- Rebuilt with `cd runtime && make build` after both code-change rounds; builds
  succeeded.
- Manually re-ran the exact prompt
  `Use fetch_url to fetch https://www.theguardian.com/us-news/2026/mar/16/trump-tariffs-absolute-right-claim-supreme-court-ruling and summarize it in one sentence.`
  on a fresh runtime. It now completed as:
  - `thinking...`
  - `fetching...`
  - `summarizing...`
  - final one-sentence answer
- Ran `./bin/check`; it passed.
- Ran `./bin/run-suite --suite m6live --config harness/config.runtime-m6.json`
  after the fetch fixes:
  - the live answer was correct
  - the harness failed only because the model performed two `search_web` calls
    instead of the currently expected one
- Ran `./bin/run-suite --suite m5live --config harness/config.runtime-m5.json`
  once and saw both cases fail, then traced that to a stale manual bridge still
  occupying port `8090`.
- Killed the stale `m5_run.py`, `m5_host_bridge.py`, and QEMU processes, then
  re-ran `./bin/run-suite --suite m5live --config harness/config.runtime-m5.json`;
  both live M5 cases passed again.
- Tightened the runtime prompt in `runtime/src/agent/loop.rs` so live research
  turns now explicitly:
  - prefer `fetch_url` after a non-empty `search_web`
  - stop with a sourced final answer after a successful supporting fetch for a
    single-fact request instead of searching again for reconfirmation
- Added dynamic prompt sections that trigger when the latest tool result is:
  - a non-empty `search_web` result set
  - a fetched-page preview
- Rebuilt with `cd runtime && make build`; it succeeded.
- Re-ran `./bin/run-suite --suite m6live --config harness/config.runtime-m6.json`;
  it passed.
- Re-ran `./bin/run-suite --suite m5live --config harness/config.runtime-m5.json`;
  it passed.
- Re-ran `./bin/check`; it passed.

## 2026-03-20 LLM I/O Logging

- Added `model_request_snapshot` / `model_response_snapshot` support in
  `runtime/src/agent/mod.rs`.
- Instrumented the main guest-side OpenAI call sites:
  - `runtime/src/agent/model.rs`
  - `runtime/src/agent/goal.rs`
  - `runtime/src/agent/loop.rs`
- Added `harness/lib/llm_log.py` to pair per-interaction request/response
  snapshots into `llm_api_log.jsonl`.
- Wired `harness/lib/run_case.py` to emit `llm_api_log.jsonl` for harness runs.
- Reworked `tools/m5_run.py` so manual runs now:
  - launch QEMU on a PTY
  - auto-enable `trace on`
  - capture UART output
  - parse trace events
  - write `uart.log`, `trace.jsonl`, and `llm_api_log.jsonl` under
    `output/m5-manual/<timestamp>/`
- Added README and harness README notes for the new manual and harness artifact.
- Ran:
  - `python3 -m py_compile tools/m5_run.py harness/lib/llm_log.py harness/lib/run_case.py`
  - `cd runtime && make build`
  - `./bin/check`
  - `./bin/run-suite --suite m6live --config harness/config.runtime-m6.json`
- Verified the live run produced
  `output/m6live-suite/m6live-search-and-answer/llm_api_log.jsonl` with paired
  request/response rows.
- Re-ran `./bin/run-suite --suite m5live --config harness/config.runtime-m5.json`
  and found one harness-only expectation mismatch, not a functional regression:
  the live case now starts with a legitimate `list_workspace`, and the model may
  use either `apply_patch` or `write_file` for the repair step.
- Updated `harness/cases/m5live-fix-small-regression/task.json` so the expected
  ordered tool subset matches the current bounded coding behavior without
  hard-coding one edit tool.
- Fixed `tools/m5_run.py` so direct execution by absolute path also works:
  the script now prepends the repository root to `sys.path` before importing
  `harness.lib.*`.
- Re-validated with:
  - `python3 tools/m5_run.py --help`
  - `python3 -m py_compile tools/m5_run.py`
- Added `tools/agent_run.py` as the canonical manual launcher and kept
  `tools/m5_run.py` as a compatibility alias.
- Renamed the manual output root from `output/m5-manual/` to
  `output/agent-manual/` in the canonical launcher, while still cleaning stale
  `output/m5-manual/current.json` state from older runs.
- Changed the launcher so guest `trace on` is no longer automatic by default.
  That turned out to be too restrictive for live log viewing, so the launcher
  now captures trace silently by default, updates `trace.jsonl` /
  `llm_api_log.jsonl` during the run, and only shows raw TRACE lines when
  `--show-trace` is passed.
- Added `tools/view_llm_log.py` to render `llm_api_log.jsonl` in readable
  plain-text or Markdown form, with support for:
  - latest-log discovery
  - single-turn selection
  - `--full` detail mode
  - Markdown export via `--output`
- Extended `tools/view_llm_log.py` with:
  - `--follow` for tail-like live viewing
  - `--follow-latest` to auto-switch when a newer latest log file appears
  - ANSI colorized plain-text rendering
  - `--color auto|always|never`
- Hardened the launcher bootstrap hiding in `tools/m5_run.py` so silent trace
  capture no longer leaks bootstrap noise into the interactive terminal:
  - the launcher now waits for the final prompt at the end of the hidden
    bootstrap block instead of restoring display on the first `Goal > `
    substring it sees
  - this suppresses `Goal > trace on`, the echoed `trace on`, and the initial
    TRACE burst while still keeping `trace.jsonl` and `llm_api_log.jsonl`
    updating in real time
- Revalidated the launcher/viewer path with:
  - `python3 -m py_compile tools/m5_run.py tools/agent_run.py tools/view_llm_log.py`
  - local PTY display-state simulations covering
    `Goal > trace on\\ntrace on\\nGoal > ` and
    `Goal > trace on\\ntrace on\\nTRACE ...\\nGoal > `
  - `./bin/check`
- Fixed a second launcher trace-leak path in `tools/m5_run.py`:
  - partial buffers that already started with `TRACE ` were previously treated
    as ordinary text once they were longer than the literal prefix
  - now those long trace fragments are also held back until a newline arrives,
    so split `TRACE {...}` rows no longer bleed into the interactive terminal
- Revalidated with:
  - `python3 -m py_compile tools/m5_run.py`
  - a local chunk-boundary simulation for long partial `TRACE {...}` rows
  - `./bin/check`
- Put the launcher stdin into cbreak mode during a live manual session so the
  host terminal stops locally echoing typed input; only the guest-side colored
  `Goal >` echo remains visible now.
- Revalidated with:
  - `python3 -m py_compile tools/m5_run.py tools/agent_run.py`
  - `./bin/check`
- Fixed `tools/view_llm_log.py --follow/--follow-latest` so it now refreshes
  when an existing row changes in place, not only when new rows are appended:
  - the viewer now snapshots row content, not just row count
  - when the same turn is rewritten from `response: null` to a full response,
    the follow view rerenders instead of staying stuck on `(missing)`
  - pure append-only growth still uses the cheaper incremental path
- Revalidated with:
  - `python3 -m py_compile tools/view_llm_log.py`
  - a local snapshot-difference check for missing-output -> full-output rows
  - `./tools/view_llm_log.py --file output/agent-manual/20260320-134944/llm_api_log.jsonl`
  - `./bin/check`
- Extended `tools/view_llm_log.py` with context-engineering views:
  - `--raw` for full request/response payload inspection
  - `--budget` for chars and rough token estimates
  - `--diff` for request/response diffs against the previous turn
- Defined Milestone 7 in `docs/milestones/m7.md` as a durable memory and
  context runtime milestone.
- Updated `README.md` and `AGENTS.md` so repository navigation now includes the
  M7 milestone document and a short M7 direction summary.
- Added `docs/milestones/m7-tool-contract.md` to define the first bounded M7
  memory surface, shared concepts, and optional checkpoint/compaction flows.
- Added `docs/milestones/m7-harness-matrix.md` to define the first fixture/live
  acceptance matrix for memory inspection, truthful compaction, follow-up
  reuse, and resume.
- Added `docs/milestones/m7-artifact-contract.md` to define the stable M7
  artifact set for memory snapshots, memory events, context snapshots, context
  budgets, and checkpoint inspection.
- Added `docs/milestones/m7-memory-backend.md` to define the first implementable
  backend split between guest working memory, guest context assembly, and
  host-side checkpoint/artifact persistence.
  - `--focus all|request|response|system|input|output` to isolate one slice
- Adjusted the default rendering so:
  - `OUTPUT` shows semantic content instead of the full compact JSON wrapper
  - `SYSTEM / TOOLS` shows the actual instructions block that is sent to the model
  - raw request/response payloads remain available via `--raw` and `--full`
- Revalidated with:
  - `python3 -m py_compile tools/view_llm_log.py`
  - `./tools/view_llm_log.py --file output/agent-manual/20260320-134944/llm_api_log.jsonl --raw --focus request`
  - `./tools/view_llm_log.py --file output/agent-manual/20260320-134944/llm_api_log.jsonl --budget --focus request`
  - `./tools/view_llm_log.py --file output/agent-manual/20260320-134944/llm_api_log.jsonl --raw --focus response --full`
  - `./tools/view_llm_log.py --file output/m5live-suite/m5live-run-process-and-read-output/llm_api_log.jsonl --diff --focus request`
  - `./bin/check`
- Hardened `tools/view_llm_log.py` against partial historical rows:
  - `request` / `response` are now treated as optional dicts
  - missing or non-string fields are rendered safely instead of crashing with
    `AttributeError: 'NoneType' object has no attribute 'get'`
  - missing response text now displays as `(missing)`
- Revalidated with:
  - `python3 -m py_compile tools/view_llm_log.py`
  - `./tools/view_llm_log.py --file output/m5live-suite/m5live-run-process-and-read-output/llm_api_log.jsonl`
  - `./tools/view_llm_log.py --file output/m5live-suite/m5live-run-process-and-read-output/llm_api_log.jsonl --markdown --output /tmp/llm-log-test.md`
  - `./bin/check`
- Investigated the manual `virtio-net tcp payload too large` failure and traced
  it to the plain-HTTP `FETCH_HTTP` path still sending entire host-bridge
  requests as one TCP payload.
- Changed `runtime/src/main.rs` so plain HTTP now mirrors the TLS request path:
  - `FETCH_HTTP_LEN` is tracked as `usize`
  - new `FETCH_HTTP_OFF` tracks per-request send progress
  - each tick sends at most `net::max_tcp_payload_len()` bytes
  - response timeouts restart the request at offset `0` instead of trying to
    blast the whole request in one retransmit
- Reset `FETCH_HTTP_OFF` in the relevant fetch-start, round-reset, and
  TCP-establishment paths so plain HTTP retries and reconnects start from a
  clean offset.
- Rebuilt with `cd runtime && make build`; it succeeded.
- Reproduced the original manual coding request through the real launcher:
  - `./tools/agent_run.py --workspace /Users/sangwf/code/MiniAgentOS`
  - prompt: `帮我写个python程序,输出9*9乘法口诀表,将运行结果发给我`
- Verified from `output/agent-manual/20260320-144928/trace.jsonl` that the
  formerly failing large host-bridge POST bodies now complete successfully:
  - `model_request_built body_len=10004`
  - `model_request_built body_len=8901`
  - both followed by `fetch_result_snapshot ok=true http_status=200`
- Verified the manual run now reaches the full bounded coding loop and renders
  the final multiplication table instead of failing with
  `virtio-net tcp payload too large`.
- Ran `./bin/check`; it passed.
- While closing the manual test session, found and fixed a separate launcher
  cleanup bug in `tools/m5_run.py` so a slow-to-die runtime no longer produces
  a Python traceback on `Ctrl-C`.
- Revalidated the launcher helper with:
  - `python3 -m py_compile tools/m5_run.py tools/agent_run.py`
- Added `harness/lib/m7_substrate.py` as the first deterministic M7 fixture
  backend. It keeps working memory in RAM and emits:
  - `memory_snapshot.json`
  - `memory_events.json`
  - `context_snapshot.json`
  - `context_budget.json`
  - `checkpoint_snapshot.json` when checkpoints exist
- Extended `harness/lib/run_case.py` to load the new M7 artifacts and pass them
  into the evaluator.
- Extended `harness/lib/evaluator.py` with M7 assertions for:
  - retained memory entries
  - memory mutation events
  - context section visibility
  - context budget fields
  - checkpoint presence and metadata
- Extended `harness/lib/run_suite.py` so `--suite m7` is supported.
- Extended `harness/fixtures/fake_agent.py` with deterministic M7 turn
  behaviors for:
  - memory inspection
  - context budget reporting
  - truthful compaction
  - follow-up after a large retained result
  - research-memory follow-up
  - coding-memory follow-up
  - checkpoint save/resume
- Added the first M7 fixture cases:
  - `m7-memory-inspection`
  - `m7-context-budget-report`
  - `m7-followup-after-large-tool-result`
  - `m7-truthful-compaction`
  - `m7-research-memory-followup`
  - `m7-coding-memory-followup`
  - `m7-resume-interrupted-task`
- Updated milestone and harness docs to reflect that M7 now has a first
  fixture-backed implementation slice:
  - `docs/milestones/m7.md`
  - `README.md`
  - `AGENTS.md`
  - `harness/README.md`
- Revalidated with:
  - `python3 -m py_compile harness/fixtures/fake_agent.py harness/lib/m7_substrate.py harness/lib/run_case.py harness/lib/evaluator.py harness/lib/run_suite.py`
  - `./bin/check`
  - `./bin/run-suite --suite m7 --config harness/config.fixture.json`
  - `./bin/run-suite --suite m6 --config harness/config.fixture.json`
  - `./bin/run-suite --suite m5 --config harness/config.fixture.json`
  - `./bin/run-suite --suite m4 --config harness/config.fixture.json`
- Confirmed no new fixture regressions:
  - `m7` passes
  - `m6` passes
  - `m5` passes
  - `m4` still only fails:
    - `m4-loop-context-bleed-repro`
    - `m4-loop-openai-followup`

## 2026-03-20 M7 Runtime Slice

- Read the existing M7 docs, fixture substrate, and current runtime
  session/prompt assembly code to find bounded hook points for a first in-guest
  memory implementation.
- Added `/Users/sangwf/code/MiniAgentOS/runtime/src/agent/memory.rs` as the
  first guest-side M7 memory module.
- Kept the first runtime slice intentionally narrow:
  - one retained slot per memory class
  - guest RAM only
  - inspection-first surfaces
- Wired the new memory module into:
  - `/Users/sangwf/code/MiniAgentOS/runtime/src/agent/mod.rs`
  - `/Users/sangwf/code/MiniAgentOS/runtime/src/agent/session.rs`
  - `/Users/sangwf/code/MiniAgentOS/runtime/src/agent/loop.rs`
- Session lifecycle is now memory-aware:
  - session reset clears memory
  - user turns retain task memory
  - tool results retain source/workspace/execution memory
  - assistant turns retain conversation memory
- Extended the guest prompt contract with new bounded sections:
  - `Working memory`
  - `Known sources`
  - `Workspace memory`
- Added first-pass context budget capture for those sections via
  `memory::record_context_budget(...)`.
- Extended the guest tool surface with three sync M7 tools:
  - `memory_status`
  - `list_memory`
  - `read_memory`
- Added matching shell commands:
  - `memory-status`
  - `memory-list [kind]`
  - `memory-read <id>`
- Rebuilt successfully with:
  - `cd /Users/sangwf/code/MiniAgentOS/runtime && make build`
- Re-ran:
  - `./bin/check`
  - `./bin/run-suite --suite m7 --config harness/config.fixture.json`
  - `./bin/run-suite --suite m6 --config harness/config.fixture.json`
- Smoke-tested the new guest shell memory commands in QEMU:
  - `memory-status` returned counts and budget JSON
  - `memory-list` returned an empty retained set on a fresh session
  - `memory-read mem-task` returned `unknown_memory_id` before any retained
    task memory existed

## 2026-03-20 M7 Live Slice

- Extended the guest runtime trace to emit live M7 memory/context signals:
  - `memory_event`
  - `memory_entry_snapshot`
  - `context_budget_snapshot`
  - per-section `context_section_snapshot`
- Extended `/Users/sangwf/code/MiniAgentOS/harness/lib/run_case.py` to
  synthesize live M7 artifacts from trace when the guest run does not emit them
  directly.
- Added `/Users/sangwf/code/MiniAgentOS/harness/config.runtime-m7.json`.
- Added the first live case:
  - `/Users/sangwf/code/MiniAgentOS/harness/cases/m7live-memory-inspection/task.json`
  - `/Users/sangwf/code/MiniAgentOS/harness/cases/m7live-memory-inspection/source.md`
- First `m7live` run failed only on harness expectations:
  - the case expected one context snapshot turn and one context-budget turn
  - the real guest loop correctly produced two turns (`memory_status` tool turn
    plus final answer turn)
- Updated the live case to expect two context snapshots and two context-budget
  snapshots.
- Re-ran:
  - `./bin/run-suite --suite m7live --config harness/config.runtime-m7.json`
  - `./bin/run-suite --suite m7 --config harness/config.fixture.json`
  - `./bin/check`
- Validation status after the fix:
  - `m7live` passed
  - `m7` passed
  - `check` passed

## 2026-03-20 M7 Guest Truthful Compaction

- Extended `/Users/sangwf/code/MiniAgentOS/runtime/src/agent/memory.rs` with a
  first automatic guest-side truthful-compaction policy for retained:
  - source results
  - workspace results
  - execution results
  - assistant responses
- Added dedicated compacted-summary builders for:
  - `search_web`
  - `fetch_url`
  - `read_process_output`
  plus a generic bounded excerpt path for workspace/tool output.
- Added `memory_compacted` trace emission with retained vs dropped size
  metadata.
- Extended `/Users/sangwf/code/MiniAgentOS/harness/lib/run_case.py` so
  trace-derived `memory_events.json` keeps `memory_compacted` events in live
  runs.
- Added a new live case:
  - `/Users/sangwf/code/MiniAgentOS/harness/cases/m7live-truthful-compaction/task.json`
  - `/Users/sangwf/code/MiniAgentOS/harness/cases/m7live-truthful-compaction/source.md`
- Verified the new live compaction case directly:
  - `./bin/run-case harness/cases/m7live-truthful-compaction/task.json --config harness/config.runtime-m7.json --output output/m7live-compaction-retest`
  - result: pass
- Re-ran full validation:
  - `cd /Users/sangwf/code/MiniAgentOS/runtime && make build`
  - `./bin/run-suite --suite m7 --config harness/config.fixture.json`
  - `./bin/run-suite --suite m7live --config harness/config.runtime-m7.json`
  - `./bin/check`
- Validation status after the compaction slice:
  - `m7` passed
  - `m7live` passed with:
    - `m7live-memory-inspection`
    - `m7live-truthful-compaction`
  - `check` passed
