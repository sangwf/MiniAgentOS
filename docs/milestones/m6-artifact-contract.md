# M6 Artifact Contract Draft

## Purpose

This document defines the first artifact contract for M6 Harness Engineering.

The goal is to make M6 evaluation deterministic by fixing:

- which artifacts every M6 run must produce
- what each artifact means
- which fields the evaluator can rely on

without depending on manual log inspection.

## Design Principles

- artifact names should remain stable across fixture and live M6 runs
- artifacts should reflect the real research loop instead of only final text
- search, fetch, and synthesis must remain distinguishable in artifacts
- multi-turn follow-up behavior must be reconstructable from saved artifacts
- the first contract should stay compact; richer artifacts can be added later

## Required Artifacts

Every M6 case should preserve at least:

- `search_results.json`
- `fetched_sources.json`
- `tool_calls.json`
- `session_transcript.json`
- `report.json`

For multi-turn follow-up cases, the first draft should also preserve:

- `source_memory.json`

## Artifact Definitions

### `search_results.json`

Purpose:

- record each `search_web` invocation
- preserve the returned candidate source set
- make snippet-level evidence auditable

Suggested top-level shape:

```json
{
  "searches": [
    {
      "turn_index": 0,
      "tool_call_index": 0,
      "query": "latest rust async runtime benchmarks",
      "top_k": 5,
      "freshness": "month",
      "results": [
        {
          "id": "r1",
          "title": "Benchmarking async runtimes in Rust",
          "url": "https://example.com/benchmarks",
          "snippet": "A comparison of tokio, async-std, and monoio...",
          "domain": "example.com",
          "rank": 1,
          "published_at": "2026-03-10"
        }
      ],
      "truncated": false
    }
  ]
}
```

Minimum required fields per search:

- `turn_index`
- `tool_call_index`
- `query`
- `results`

Minimum required fields per result:

- `id`
- `title`
- `url`
- `snippet`
- `domain`
- `rank`

### `fetched_sources.json`

Purpose:

- record which URLs were actually fetched
- preserve the distinction between search results and fetched evidence
- let the evaluator prove that answers came from fetched pages, not snippets

Suggested top-level shape:

```json
{
  "sources": [
    {
      "turn_index": 0,
      "tool_call_index": 1,
      "search_result_id": "r1",
      "url": "https://example.com/benchmarks",
      "status_code": 200,
      "content_type": "text/html",
      "content_preview": "Tokio showed the lowest tail latency...",
      "truncated": false
    }
  ]
}
```

Minimum required fields per source:

- `turn_index`
- `tool_call_index`
- `url`
- `content_preview`
- `truncated`

Optional but strongly recommended:

- `search_result_id`
- `status_code`
- `content_type`

### `tool_calls.json`

Purpose:

- preserve ordered tool activity for evaluator checks
- let the harness assert that search and fetch were both used when required

This file already exists in earlier milestones and should be extended, not
replaced.

M6-specific minimum expectations:

- every `search_web` call is recorded
- every `fetch_url` call used in an M6 case is recorded
- error results remain visible for denied or unsupported search requests

### `session_transcript.json`

Purpose:

- preserve per-turn interaction state
- make follow-up evaluation possible

This file already exists in earlier milestones and should continue to act as
the canonical turn-level record.

M6-specific minimum expectations:

- user requests are preserved per turn
- assistant terminal outputs are preserved per turn
- turn-level tool call references or summaries remain available

### `report.json`

Purpose:

- preserve final evaluator pass/fail state
- summarize the key research behaviors checked for that case

The first M6 report can remain compact, but it should expose enough metadata to
answer:

- did search happen
- did fetch happen
- was multi-source behavior required
- was follow-up required
- did the evaluator accept or reject the run

## Follow-up Artifact

### `source_memory.json`

Purpose:

- make the runtime’s bounded known-source set visible across turns
- support evaluator checks for follow-up reuse

This artifact is required for follow-up-oriented M6 cases and optional for
single-turn cases.

Suggested top-level shape:

```json
{
  "sources": [
    {
      "id": "r1",
      "url": "https://example.com/benchmarks",
      "domain": "example.com",
      "from_search": true,
      "fetched": true,
      "first_seen_turn": 0,
      "last_used_turn": 1
    }
  ]
}
```

Minimum required fields:

- `id`
- `url`
- `from_search`
- `fetched`
- `first_seen_turn`
- `last_used_turn`

## Optional Later Artifacts

The first M6 slice does not require these, but the contract should leave room
for them:

- `research_notes.json`
- `search_provider_trace.json`
- `fetched_source_bodies/<source_id>.txt`

These can help debugging, but they should not be required for the first harness
bar.

## Evaluator Assumptions

The first M6 evaluator should be able to answer these questions from artifacts
alone:

1. Did the runtime call `search_web`?
2. Did it fetch one or more selected URLs?
3. Was the fetched source set empty or non-empty?
4. For multi-source cases, were at least two distinct sources fetched?
5. For follow-up cases, did the runtime preserve a truthful known-source set?
6. For refusal cases, did the runtime stay explicit about missing or
   insufficient evidence?

## Fixture And Live Consistency

The same artifact names and broad field meanings should be used for:

- fixture-backed M6 cases
- real QEMU-backed live M6 cases

Live runs may add extra fields, but they should not remove the minimum required
fields that fixture evaluation depends on.

## First-Draft Contract Summary

The first M6 harness should be considered artifact-complete when it can
reliably emit:

- `search_results.json`
- `fetched_sources.json`
- `tool_calls.json`
- `session_transcript.json`
- `report.json`

and, for follow-up cases:

- `source_memory.json`
