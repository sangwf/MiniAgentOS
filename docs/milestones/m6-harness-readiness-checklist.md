# M6 Harness Readiness Checklist

## Purpose

This document captures the decisions that should be fixed before implementing
the first M6 Harness Engineering slice.

The goal is to prevent M6 work from drifting because the search backend,
artifact contract, or evaluation policy is still ambiguous.

## Confirmed Decisions

### 1. Milestone Shape

The first M6 slice is bounded web search and research, not browser automation.

This means:

- search and fetch remain separate capabilities
- the milestone bar is research-oriented, not web automation-oriented
- the live implementation does not require a full browser stack

### 2. Live Search Provider

The first live M6 provider is Brave Search.

This means:

- the host side reads `BRAVE_API_KEY`
- the guest contract remains provider-neutral
- live search requests should not scrape search result pages directly

### 3. Search Tool Surface

The required first-draft M6 tool surface is intentionally narrow.

Required:

- `search_web`

Reused existing capability:

- `fetch_url`

Optional later tools:

- `list_sources`
- `read_source`
- `clear_sources`

## Remaining Harness Decisions

The following items are now fixed as the default first-draft policy.

### 4. Artifact Contract

The first M6 harness preserves at least:

- `search_results.json`
- `fetched_sources.json`
- `tool_calls.json`
- `session_transcript.json`
- `report.json`

Required for follow-up cases:

- `source_memory.json`

### 5. Backend Split

The first M6 harness uses two search backends:

- deterministic fixture backend for the core acceptance bar
- Brave-backed live backend for the live bar

Accepted rule:

- fixture cases establish the core behavioral bar before live search is used as
  a milestone-completion gate

### 6. Search Policy

Accepted first-draft bounds:

- max query length: 512 bytes
- max `top_k`: 10
- max domain allowlist entries: 10
- max domain denylist entries: 10
- max returned snippet length per result: 512 bytes

If later cases need broader search shapes, add them as explicit extensions.

### 7. Evidence Policy

Accepted first-draft evaluator rule:

- search snippets are not sufficient evidence when a case expects fetched page
  content

This means the evaluator should reject answers that only mirror snippets when
the case requires a `fetch_url` step.

### 8. Follow-up State Policy

Accepted first-draft rule:

- follow-up-oriented M6 cases must preserve truthful source state across turns

This means:

- `session_transcript.json` remains required
- `source_memory.json` is required for follow-up cases

### 9. Live Secret Handling

Accepted first-draft rule:

- the live M6 path reads `BRAVE_API_KEY` from the host environment

This matches the current repository pattern where live secrets are injected from
shell environment instead of hardcoded config files.

### 10. M4/M5 Coexistence

Accepted rule:

- M6 harness work must preserve M4 and M5 validity

M6 introduces new suites and artifacts. It must not reinterpret the existing
M4/M5 acceptance bars.

## Recommended First Acceptance Slice

The first M6 fixture slice should target exactly these cases:

- `m6-search-and-answer`
- `m6-search-no-results`
- `m6-snippet-is-not-evidence`
- `m6-compare-two-sources`
- `m6-search-then-followup`
- `m6-insufficient-evidence-refusal`

The first live M6 slice should then require:

- one real QEMU-backed `search -> fetch -> answer` case
- one real QEMU-backed follow-up case reusing source state

## Ready-To-Implement Checklist

M6 harness implementation is ready to begin when all of the following are true:

- bounded research scope is accepted
- Brave live provider is accepted
- search tool surface is accepted
- artifact contract is accepted
- fixture/live backend split is accepted
- search policy bounds are accepted
- evidence policy is accepted
- follow-up state policy is accepted
- secret handling is accepted
- M4/M5 coexistence rule is accepted

## Why This Checklist Matters

If these decisions remain implicit, M6 implementation effort will fragment
across:

- bridge design
- fixture design
- search artifact generation
- evaluator rules
- live provider plumbing

Fixing these decisions first keeps M6 aligned with the repository’s existing
Harness Engineering discipline.
