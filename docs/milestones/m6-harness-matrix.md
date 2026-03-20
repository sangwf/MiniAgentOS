# M6 Harness Case Matrix Draft

## Purpose

This document defines the first Harness Engineering case surface for M6.

The goal is to validate that MiniAgentOS can complete a bounded research loop:

`search -> inspect results -> fetch sources -> compare -> summarize -> follow-up`

through the M6 search contract and the existing fetch/session substrate.

## Acceptance Strategy

M6 should not be accepted by one ad hoc demo where a live search happens to
work.

It should be accepted by a compact harness matrix that proves:

- search is real
- search and fetch remain distinct
- fetched evidence is tracked separately from snippets
- follow-up turns can reuse known sources
- the runtime can refuse or qualify unsupported research states honestly

## Fixture And Live Split

The first M6 matrix should distinguish:

- **fixture cases**
  deterministic search and source fixtures for contract validation
- **live cases**
  real QEMU-backed runs using a Brave-backed search bridge with
  `BRAVE_API_KEY`

Fixture cases should establish the core behavioral bar before live search is
used as a milestone-completion gate.

## Case Groups

### Group A: Search Fundamentals

These cases prove the runtime can issue a bounded search and reason about the
returned candidate sources.

#### `m6-search-and-answer`

Goal:

- agent performs one search
- agent chooses a result to inspect
- agent fetches the chosen source
- agent answers from fetched evidence

Expected behaviors:

- at least one `search_web`
- at least one `fetch_url`
- no unsupported shortcut that skips fetching and answers only from snippets

Harness assertions:

- `tool_calls.json` includes both `search_web` and `fetch_url`
- `search_results.json` records the returned candidates
- `fetched_sources.json` records the selected URL
- final answer matches expected evidence-backed content

Suggested fixture:

- deterministic search backend returns 3 structured results
- only one fetched page contains the answer

#### `m6-search-no-results`

Goal:

- agent performs one search
- search returns zero results
- agent responds honestly without fabrication

Expected behaviors:

- `search_web`
- no `fetch_url`

Harness assertions:

- search result set is empty
- final result is a bounded refusal or "not enough evidence" answer
- no fetched sources are recorded

Suggested fixture:

- deterministic search backend always returns `results: []`

### Group B: Search And Fetch Separation

These cases prove the runtime keeps snippets and fetched evidence distinct.

#### `m6-snippet-is-not-evidence`

Goal:

- agent performs a search whose snippets are suggestive but incomplete
- agent must fetch the real page before answering

Expected behaviors:

- `search_web`
- `fetch_url`

Harness assertions:

- final answer reflects page content, not just snippet text
- fetched source set is non-empty
- snippet-only answering does not satisfy the evaluator

Suggested fixture:

- search snippets hint at a claim but the full page clarifies the exact answer

#### `m6-compare-two-sources`

Goal:

- agent searches once
- fetches two distinct sources
- compares them and summarizes the difference

Expected behaviors:

- `search_web`
- at least two `fetch_url` calls

Harness assertions:

- `fetched_sources.json` contains two distinct URLs
- final answer reflects both sources
- evaluator rejects answers that only mention one source

Suggested fixture:

- two pages disagree on one detail or offer complementary evidence

### Group C: Research Follow-up

These cases prove the session loop can preserve research state.

#### `m6-search-then-followup`

Goal:

- first turn performs a search and fetches at least one source
- second turn asks a follow-up question using the same research thread

Expected behaviors:

- turn 1 uses `search_web` and `fetch_url`
- turn 2 may reuse known sources, optionally with more fetches

Harness assertions:

- session transcript shows a true follow-up conversation
- turn 2 has access to a truthful known-source set
- evaluator confirms the answer is consistent with prior fetched evidence

Suggested fixture:

- first turn asks for "What happened?"
- second turn asks for "Summarize only the performance claims."

#### `m6-followup-without-refetch`

Goal:

- agent searches and fetches in the first turn
- second turn asks a narrow follow-up that should be answerable from known
  sources

Expected behaviors:

- turn 1 uses `search_web` and `fetch_url`
- turn 2 may answer from known sources without a new fetch

Harness assertions:

- known-source reuse is visible in session artifacts
- final answer remains consistent with earlier fetched evidence

Suggested fixture:

- first turn gathers one or two sources
- second turn asks for a shorter reformulation or language/style transformation

### Group D: Honest Refusal And Boundedness

These cases prove the runtime stays policy-controlled.

#### `m6-insufficient-evidence-refusal`

Goal:

- agent searches and fetches
- available evidence is insufficient to answer the user’s stronger claim
- agent says so instead of inventing unsupported facts

Expected behaviors:

- `search_web`
- `fetch_url`
- bounded refusal or qualified answer

Harness assertions:

- evaluator accepts explicit uncertainty
- evaluator rejects fabricated certainty

Suggested fixture:

- search returns pages that discuss the topic but do not support the exact
  requested conclusion

#### `m6-deny-oversized-domain-filter`

Goal:

- agent attempts a search request outside allowed search policy
- runtime denies it in a structured way

Expected behaviors:

- `search_web`
- no fetch required

Harness assertions:

- tool result includes a structured policy error
- `tool_calls.json` captures the denied call
- final answer reports the limitation instead of crashing

Suggested fixture:

- query includes an excessive domain allowlist or other forbidden filter shape

## Recommended First Acceptance Bar

The first M6 implementation should require these fixture cases:

- `m6-search-and-answer`
- `m6-search-no-results`
- `m6-snippet-is-not-evidence`
- `m6-compare-two-sources`
- `m6-search-then-followup`
- `m6-insufficient-evidence-refusal`

The first live M6 bar should then require at least:

- one real QEMU-backed `search -> fetch -> answer` case
- one real QEMU-backed follow-up case reusing known sources

## Artifact Expectations

The first M6 harness matrix should expect at least:

- `search_results.json`
- `fetched_sources.json`
- `tool_calls.json`
- `session_transcript.json`
- `report.json`

Optional later artifacts:

- `source_memory.json`
- `research_notes.json`
- `search_provider_trace.json`

## Evaluation Priorities

M6 evaluation should prioritize:

1. whether the runtime really searched
2. whether it really fetched selected sources
3. whether the answer reflects fetched evidence rather than snippet-only guesswork
4. whether follow-up turns preserve truthful research state
5. whether the runtime stays honest when evidence is weak or missing
