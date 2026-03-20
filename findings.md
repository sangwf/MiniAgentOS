# Findings

## 2026-03-17

- `docs/milestones/m5.md` is implemented and positions M5 as a bounded coding substrate.
- There is currently no M6 documentation or mention in `README.md`, `docs/`, or `AGENTS.md`.
- The user wants M6 to focus on web search rather than more coding capability.
- The preferred live secret is `BRAVE_API_KEY`, expected from shell startup files such as `.zshrc`.
- Official Brave Search API docs position Brave as a standalone web search API with its own index and support for freshness filtering and extra snippets.
- Tavily remains a viable alternative, but it is more answer/content-oriented; Brave is the cleaner fit if MiniAgentOS should own the `search -> fetch -> synthesize` loop itself.
- Google Custom Search JSON API is not a good M6 starting point because its availability is limited and the official docs point to a 2027 shutdown path.
- Bing Web Search API is retired, so it should not be considered for M6.
- The chosen M6 shape is: bounded search in a host-backed bridge, with `search_web` as the only required new tool and `fetch_url` reused for source reading.
- The chosen live secret is `BRAVE_API_KEY`, read from the user's shell environment.
- The first M6 tool contract should stay narrow: require `search_web`, reuse `fetch_url`, and leave `list_sources` / `read_source` / `clear_sources` as optional follow-on tools.
- The contract should preserve a strict distinction between search-result metadata and fetched source content so the harness can evaluate truthful research behavior.
- The first M6 harness bar should be split into deterministic fixture cases and a smaller live Brave-backed bar.
- The most important evaluator checks for M6 are not "did search run" but "did the agent fetch evidence" and "did follow-up turns reuse truthful source state."
- M6 still lacked a concrete artifact contract; this is now defined separately so fixture/live runs can share stable artifact names and minimum fields.
- `source_memory.json` should be required for follow-up-oriented M6 cases, not only treated as a nice-to-have debug file.
- The remaining ambiguity after the first three M6 docs was implementation readiness. That gap is now closed by a backend/bridge draft and a harness readiness checklist.
- The current repository state for M6 is strong enough to begin fixture/backend/evaluator implementation without redefining milestone scope again.
- The first implemented M6 slice uses a deterministic fixture search backend inside the fixture agent plus multi-page source fixtures served by `start_source_fixture`.
- New harness artifacts now include `search_results.json`, `fetched_sources.json`, and `source_memory.json`.
- `run_case.py` now passes search fixture paths and source-base URLs into the fixture agent and loads M6 artifacts back into the evaluator.
- `evaluator.py` now supports search-count, fetched-source-count, and source-memory assertions.
- `run_suite.py` now accepts `--suite m6`.
- The first M6 fixture suite currently includes:
  - `m6-search-and-answer`
  - `m6-search-no-results`
  - `m6-snippet-is-not-evidence`
  - `m6-compare-two-sources`
  - `m6-search-then-followup`
  - `m6-insufficient-evidence-refusal`
- The first live M6 slice is now implemented through the existing host bridge:
  - `tools/m5_host_bridge.py` now serves Brave-backed `/search/web`
  - `runtime/src/agent/loop.rs` now exposes `search_web` to the real M4/M5/M6 loop
  - `harness/config.runtime-m6.json` and `harness/cases/m6live-search-and-answer/` provide the first QEMU-backed live validation path
- On this machine, Brave's official API endpoint can be flaky via direct curl/TLS, so the host bridge now falls back to `https://search.brave.com/search` HTML parsing when needed.
- Post-bridge OpenAI routing needs to be tool-sensitive:
  - `search_web` follow-up turns are more reliable on the guest-native OpenAI path
  - coding-oriented bridge tools (`list_workspace`, `read_file`, `apply_patch`, `run_process`, `read_process_output`) are more reliable when follow-up turns switch back to the host OpenAI relay
- Even after transport routing was corrected, `m5live-fix-small-regression` could still fail intermittently because the model sometimes emitted a non-JSON planning aside such as `(Will fix add to return a+b and rerun check.py)`. A bounded parse-error retry in the M4 loop removes that flake without weakening the JSON tool contract.
- Regression status after implementation:
  - `m6live` suite passes
  - `m5live` suite passes
  - `m6` fixture suite passes
  - `m5` fixture suite passes
  - `m4` fixture suite remains at the previous baseline with the same two known failures

## 2026-03-18

- Manual M6 follow-up fetches against external news sites can still fail in the
  guest-native HTTPS path even when `search_web` succeeds.
- The concrete failure reproduced against
  `https://www.theguardian.com/us-news/2026/mar/16/trump-tariffs-absolute-right-claim-supreme-court-ruling`
  is `MBEDTLS_ERR_SSL_INVALID_RECORD` (`-0x7200`) during `https_read`, after the
  handshake has already reached `handshake_over`.
- In the failing traces, `http_status` remains `0`, so the invalid-record fault
  happens before any HTTP response bytes are successfully parsed.
- The current TLS pending-segment path still used an older policy than the fixed
  plain-HTTP path: future segments were bounded in small fixed slots and exact-seq
  retransmits were previously dropped wholesale even if a later retransmit
  carried more bytes.
- A first hardening round is now in place:
  - `TLS_RX_BUF` grew from `64 KiB` to `256 KiB`
  - pending slots grew from `16` to `32`
  - pending segment max length grew from `2048` to `4096`
  - exact-seq future retransmits now replace the older pending copy if the new
    payload is longer
  - `tls-status` now reports RX compaction/overflow counters and pending-store,
    replace, duplicate-drop, oversize-drop, no-slot-drop, and replay stats
  - `tls::error_label()` now recognizes `invalid_record`
- After this hardening round, the original Guardian repro did not immediately
  repeat as `invalid_record`; the same manual scenario instead advanced into a
  later retry path and finished as `tcp connect timed out`, which indicates the
  receive-path behavior changed but the external-site fetch problem is not fully
  solved yet.

## 2026-03-19

- The Guardian article repro is much larger than the old `AGENT_RESPONSE_BODY`
  limit:
  - host-side check with `Accept-Encoding: identity` returned `Content-Length:
    315640`
  - the old agent response capture limit was `65536`
- Two independent issues were causing the apparent external-site `fetch_url`
  failures after the first TLS receive-path hardening:
  - the agent-side response buffer was too small, which turned successful large
    page fetches into `response body truncated`
  - `capture_redirect()` matched any `location:` substring, so Guardian's
    `onion-location:` header was falsely treated as a redirect target and caused
    a second fetch to a `.onion` URL that eventually timed out
- The direct raw shell fetch path against the Guardian repro now succeeds and
  returns `HTTP/1.1 200 OK` plus the full page body, which confirms the
  guest-native HTTPS transport itself is no longer dying at `invalid_record`
  for that site.
- The remaining Guardian/manual M6 blocker was therefore no longer pure TLS:
  once the transport was healthy enough to fetch the page, the runtime still
  needed larger agent capture and stricter redirect-header parsing.
- The final targeted fixes were:
  - increase `AGENT_RESPONSE_BODY` from `65536` to `524288`
  - treat truncated `fetch_url` responses with a non-empty successful body as
    usable for continuing the loop instead of immediately failing the tool
  - only recognize `Location:` when it starts a header line, which prevents
    `onion-location:` from triggering a false redirect
- After those fixes, the exact manual prompt
  `Use fetch_url to fetch https://www.theguardian.com/us-news/2026/mar/16/trump-tariffs-absolute-right-claim-supreme-court-ruling and summarize it in one sentence.`
  now completes successfully with a one-sentence summary instead of failing in
  `fetch_url`.
- `m5live` initially failed after the manual tests, but that was environmental
  contamination rather than a code regression:
  - the old manual `m5_run.py` bridge was still bound on port `8090`
  - rerunning after killing the stale `m5_run.py` / bridge / QEMU processes
    restored `m5live` to passing
- `m6live` remains functionally healthy after the fetch fixes, but the current
  live case can still fail its harness on a model-behavior detail
  (`unexpected search count: 2 != 1`) when the model performs one redundant
  second `search_web` before fetching the same Brave documentation page.
- The redundant second `search_web` was not a transport problem. After the first
  successful `search_web` and `fetch_url`, the model already had enough
  evidence, but the runtime prompt still left too much freedom for "extra
  confirmation" behavior.
- Tightening the M4/M5/M6 instructions plus adding dynamic prompt sections for:
  - non-empty `search_web` results
  - successful fetched-page previews
  is enough to make the live case stop after one search and one supporting
  fetch.
- After that prompt tightening, `m6live-search-and-answer` now passes again
  with the intended behavior:
  - one `search_web`
  - one `fetch_url`
  - final sourced answer

## 2026-03-20

- The runtime did not previously preserve a direct host-side artifact for
  guest-side LLM request/response pairs, which made it difficult to study how
  `Current request`, `Latest tool result`, `Session state`, and `Recent
  conversation` were assembled across turns.
- The right place to capture this is the existing runtime trace surface, not a
  second network side channel:
  - guest code now emits `model_request_snapshot` and
    `model_response_snapshot`
  - host code pairs those snapshots into `llm_api_log.jsonl`
- The request/response snapshots now cover the main guest OpenAI call sites:
  - goal interpretation
  - session loop model turns
  - summary-model turns
- `tools/m5_run.py` now launches the runtime through a PTY, auto-sends
  `trace on` after the first `Goal >`, mirrors UART back to the user, and saves
  three manual artifacts:
  - `uart.log`
  - `trace.jsonl`
  - `llm_api_log.jsonl`
- The new `llm_api_log.jsonl` artifact is already useful for studying memory
  behavior because each row includes:
  - full model instructions
  - the fully assembled input context
  - model name, reasoning effort, and output-token cap
  - parsed or raw model response text
- A fresh `m6live` run confirmed the new artifact works end-to-end and shows the
  stepwise evolution from:
  - initial user request
  - search-result-conditioned follow-up turn
  - fetched-source-conditioned final-answer turn
- A fresh `m5live` regression did not reveal a logging bug. The only failure was
  an outdated case expectation: the live coding model now legitimately performs
  an initial `list_workspace` before the first `run_process`, and it may choose
  either `apply_patch` or `write_file` for the bounded edit step. The case's
  expected ordered tool subset needed to be updated to match the real contract
  instead of one edit strategy.
- `tools/m5_run.py` could still fail for direct absolute-path invocation with
  `ModuleNotFoundError: No module named 'harness'` because Python only seeded
  `sys.path` with the script directory (`tools/`), not the repository root.
  The launcher now prepends the repo root explicitly before importing
  `harness.lib.*`.
- The launcher name `m5_run.py` had become misleading once the same path was
  used for both M5 coding and M6 research. The canonical manual entrypoint is
  now `tools/agent_run.py`, while `tools/m5_run.py` remains only as a
  compatibility alias.
- Auto-enabling `trace on` for every manual run was too noisy for normal use.
  The right balance is not "trace fully off", but "trace captured silently":
  keep manual interaction clean by hiding raw TRACE lines, while still enabling
  the log artifacts and live viewer updates by default.
- Raw `llm_api_log.jsonl` is too awkward to read directly in editors such as
  Cursor because each row is one long JSON object. A dedicated host-side viewer
  is the right solution; it should default to a readable turn-by-turn view and
  optionally emit Markdown for editor-friendly inspection.
- For terminal use, the viewer also needs a tail-like mode and visual cues.
  Static pretty-printing is not enough when the user wants to watch context
  assembly as new turns arrive.
- Watching only the current file is still not enough for real manual use
  because every agent restart rotates to a new `output/agent-manual/<timestamp>`
  directory. The viewer needs a "follow latest" mode that can switch files
  automatically after a restart.
- The remaining startup-noise bug was not that trace capture was "still on" in
  principle, but that the launcher bootstrap treated the first `Goal >` seen
  after sending `trace on` as the clean prompt to show again. In real PTY
  output, the bootstrap often arrives as a combined chunk like
  `Goal > trace on\ntrace on\nTRACE ...\nGoal > `, so the viewer needs to wait
  for the final prompt at the end of that hidden bootstrap block, not the first
  prompt substring.
- `llm_api_log.jsonl` rows are not guaranteed to have both `request` and
  `response` as dicts. Older or incomplete rows can legitimately contain
  `response: null`, so the viewer must treat those fields as optional and
  render a placeholder instead of assuming a strict schema.
- The terminal trace leak had a second cause beyond bootstrap prompt handling:
  long `TRACE {...}` lines could be split across PTY chunks before the newline
  arrived, and the launcher's partial-buffer path treated those already-long
  fragments as ordinary display text. Partial lines that already start with
  `TRACE ` must also be held back until the newline arrives.
- The duplicate black/green command echo was separate from trace handling. The
  black line came from the host terminal's own cooked-mode echo, while the
  green line came from the guest `Goal >` shell. The launcher needs to put the
  host stdin into cbreak/no-local-echo mode during the session so only the
  guest-side echo remains visible.
- The "viewer did not refresh" bug was in the viewer, not the log writer. A
  manual LLM turn is first written as a row with a request and no response, and
  then rewritten in place with the response once the matching
  `model_response_snapshot` arrives. The old `--follow` logic only watched row
  count, so it missed in-place row updates where the line count stayed the same.

## 2026-03-20 M7 Runtime Slice

- The current runtime prompt assembly in `runtime/src/agent/loop.rs` was still
  a bounded prompt assembler, not an explicit memory runtime: it only had
  `Current request`, `Latest tool result`, `Session state`, and
  `Recent conversation`.
- The right first in-guest M7 slice is additive and inspection-first, not a
  full durable checkpoint system:
  - keep working memory in guest RAM
  - retain one bounded slot per memory class (`task`, `source`, `workspace`,
    `execution`, `conversation`)
  - expose memory through shell commands and sync M4 tools before attempting
    live harness coverage
- A new `runtime/src/agent/memory.rs` module is now in place and wired into
  session lifecycle updates:
  - `session_reset()` clears memory
  - user turns update task memory
  - tool results update source/workspace/execution memory
  - assistant turns update conversation memory
- The real guest prompt contract now includes three additional bounded sections:
  - `Working memory`
  - `Known sources`
  - `Workspace memory`
- The runtime also records first-pass context budget counters for those
  sections, exposed through `memory_status`.
- New in-guest shell inspection commands are now available:
  - `memory-status`
  - `memory-list [kind]`
  - `memory-read <id>`
- Matching sync M4 tools are also now callable by the model:
  - `memory_status`
  - `list_memory`
  - `read_memory`
- The first smoke test of the new runtime slice succeeded:
  - `cd runtime && make build` passed
  - `memory-status`, `memory-list`, and `memory-read mem-task` produced correct
    JSON responses inside a real QEMU guest
- Existing deterministic harness baselines still hold after the runtime wiring:
  - `./bin/check` passed
  - `./bin/run-suite --suite m7 --config harness/config.fixture.json` passed
  - `./bin/run-suite --suite m6 --config harness/config.fixture.json` passed

## 2026-03-20 M7 Definition

- The most appropriate next milestone after M6 is not "more tools" but
  explicit, durable, inspectable runtime memory.
- M7 should formalize memory as a runtime-owned substrate, not leave it as a
  side effect of concatenating `Latest tool result`, `Known sources`, and
  `Recent conversation`.
- The key M7 capability areas are:
  - explicit memory classes
  - truthful compaction
  - context budgeting
  - memory inspection
  - durable resume
- M7 should stay additive on top of M4/M5/M6 and should not require a large
  new planner-visible tool family on day one.
- The first M7 doc set can start with the main milestone definition only; tool
  contract and harness matrix should come after the high-level direction is
  accepted.
- The first M7 tool contract should stay inspection-first:
  - `list_memory`
  - `read_memory`
  - `memory_status`
  with `compact_memory`, `save_checkpoint`, and `resume_checkpoint` as bounded
  follow-on surfaces rather than the required minimum.
- The first M7 harness bar should not accept "longer prompts" as success. It
  should explicitly validate:
  - memory inspection
  - context budget visibility
  - truthful compaction
  - research follow-up reuse
  - coding follow-up reuse
  - resume continuity
- The key storage decision for M7 is now explicit: working memory should live in
  guest runtime memory first, while files should be used only for checkpoints
  and host-visible artifacts.
- The new M7 artifact contract requires the harness to reason about:
  - retained memory entries
  - memory mutations over time
  - prompt assembly sections
  - prompt-layer budgets
  - checkpoint save/restore state
- The new M7 backend draft keeps M7 additive by layering explicit working
  memory on top of the existing session history and session-state mechanisms
  instead of replacing them outright.
- For context engineering, the most useful distinction is:
  - normalized view for fast reading
  - raw request/response payloads for truthful API inspection
  - per-turn diffs and budget estimates for understanding how prompt state grows
    across turns
- The `virtio-net tcp payload too large` failure on manual coding prompts was
  not a model bug; it was a plain-HTTP transport bug. `FETCH_HTTP` still sent
  the whole HTTP request in one `send_tcp()` call, unlike the TLS path which
  already streamed request bytes incrementally.
- The failing repro generated host-bridge POST bodies larger than the virtio
  transmit payload budget:
  - a traced post-tool model request was `10004` bytes
  - a later follow-up model request was `8901` bytes
  Both now succeed because plain HTTP requests are segmented by
  `net::max_tcp_payload_len()`.
- After the segmentation fix, the exact manual repro
  `帮我写个python程序,输出9*9乘法口诀表,将运行结果发给我`
  completes end-to-end:
  - `list_workspace`
  - `write_file`
  - `run_process`
  - `read_process_output`
  - final rendered multiplication table
- Manual `Ctrl-C` shutdown exposed a separate launcher cleanup bug: if QEMU did
  not exit quickly after `terminate()`, `tools/m5_run.py` could raise a second
  `TimeoutExpired` during `kill()`. The cleanup path now degrades to a warning
  instead of printing a Python traceback.
- The first useful M7 implementation can stay fixture-side. A deterministic
  memory substrate plus stable artifacts is already enough to validate explicit
  memory retention, truthful compaction, context budgeting, follow-up reuse,
  and checkpoint resume.
- The M7 evaluator only needed five new artifact families to become useful:
  - `memory_snapshot.json`
  - `memory_events.json`
  - `context_snapshot.json`
  - `context_budget.json`
  - `checkpoint_snapshot.json`
- Existing harness structure was already sufficient for M7. No new fixture
  servers or sink/source protocol changes were needed; the main work was:
  - loading new output artifacts in `run_case.py`
  - asserting them in `evaluator.py`
  - emitting them deterministically from the fixture agent
- The current M7 context split works cleanly when:
  - `task`, `execution`, and `conversation` entries feed `Working memory`
  - `source` entries feed `Known sources`
  - `workspace` entries feed `Workspace memory`
  This kept the artifact story and evaluator assertions straightforward.
- The first passing M7 fixture suite is broad enough to validate all major
  memory behaviors without live runtime support:
  - memory inspection
  - context budget reporting
  - follow-up after a compacted large result
  - truthful compaction inspection
  - research-memory follow-up
  - coding-memory follow-up
  - checkpoint save/resume

## 2026-03-20 M7 Live Slice

- The first live M7 slice works best as an inspection path, not as a fake
  long-task demo. The guest already has enough signal to expose truthful memory
  state through `memory_status`, and the harness can reconstruct the first live
  M7 artifacts from trace without inventing a new host bridge.
- The guest runtime now emits enough M7-specific trace to synthesize live
  artifacts:
  - `memory_event`
  - `memory_entry_snapshot`
  - `context_budget_snapshot`
  - per-section `context_section_snapshot`
- `run_case.py` can now synthesize these live artifacts from trace when the
  guest does not write them directly:
  - `memory_snapshot.json`
  - `memory_events.json`
  - `context_snapshot.json`
  - `context_budget.json`
- The first live case, `m7live-memory-inspection`, needs two model turns, not
  one:
  - one turn to call `memory_status`
  - one follow-up turn to summarize the observed counts/budget
  The original harness failure was therefore expectation drift, not a runtime
  bug.
- After aligning the live case expectations with the real `model -> tool ->
  model` loop, the first live M7 suite now passes:
  - `./bin/run-suite --suite m7live --config harness/config.runtime-m7.json`

## 2026-03-20 M7 Guest Truthful Compaction

- The first guest-side M7 runtime slice originally retained long tool results
  too literally:
  - `summary` was generic
  - `detail` was just a bounded raw copy
  - `state=compacted` was not meaningfully represented in live guest memory
- The right first truthful-compaction shape in the guest is policy-driven and
  additive:
  - automatically compact long retained results instead of exposing a new free
    write path
  - keep compaction explicit and inspectable
  - preserve a bounded excerpt plus an explicit statement that the full raw
    content was not carried forward
- The guest runtime now performs automatic compaction for large retained:
  - source results
  - workspace results
  - execution results
  - assistant responses
- `search_web`, `fetch_url`, and `read_process_output` now get dedicated
  compaction summaries instead of a generic truncation story:
  - search results retain top-result metadata and omit the raw JSON body
  - fetched sources retain an excerpt and explicitly note that the full page
    body was not carried forward
  - process outputs retain exit code plus stdout/stderr previews and explicitly
    note that the full raw process output was not carried forward
- The guest trace now emits `memory_compacted` with:
  - `entry_id`
  - `kind`
  - `mode`
  - `source_chars`
  - `retained_chars`
  - `dropped_chars`
- `run_case.py` now preserves those live compaction events inside
  `memory_events.json`, which makes live truthful-compaction assertions
  possible.
- A new live case,
  `harness/cases/m7live-truthful-compaction/`,
  proves the first guest compaction slice against a real QEMU guest:
  - fetch a long source fixture
  - inspect `mem-source`
  - verify the retained source memory is `state=compacted`
  - verify the key fact `ORBIT-42` survives compaction truthfully
- Validation after this change:
  - `cd runtime && make build` passed
  - `./bin/run-suite --suite m7 --config harness/config.fixture.json` passed
  - `./bin/run-suite --suite m7live --config harness/config.runtime-m7.json`
    passed
  - `./bin/check` passed
