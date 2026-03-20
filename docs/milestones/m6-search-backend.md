# M6 Search Backend And Bridge Draft

## Purpose

This document defines the first implementable backend shape for M6 search.

The goal is to realize the M6 bounded research substrate using:

- a runtime-owned `search_web` tool contract inside the guest
- a host-backed search bridge
- deterministic fixture search backends
- a live Brave-backed search adapter authenticated with `BRAVE_API_KEY`

## High-Level Model

The first M6 implementation should split responsibility into two planes:

- guest control plane
- host retrieval plane

### Guest Control Plane

Lives inside MiniAgentOS and owns:

- session loop
- tool contract
- policy-visible argument validation
- trace emission
- integration of search results into session state

### Host Retrieval Plane

Lives outside the guest and owns:

- search provider access
- provider normalization
- fixture search responses
- optional source-memory artifact support

The guest should never scrape search result pages directly. It should only see
the normalized tool contract.

## Main Components

### 1. Guest M6 Tool Layer

Extends the current loop with:

- `search_web`

Responsibilities:

- validate tool-call argument shape
- enforce guest-visible policy bounds
- send structured requests to the host bridge
- emit trace for query, results, and failures
- store bounded known-source state for later turns

### 2. Search Bridge

The host bridge is the boundary transport between MiniAgentOS and the host-side
search backend.

Responsibilities:

- accept structured search requests from the guest
- dispatch to either a fixture backend or a live backend
- normalize provider responses into the M6 tool contract
- return structured success or failure payloads
- emit or persist artifact data for harness evaluation

The bridge is not a hidden planner. It should not summarize results or answer
the user’s question.

### 3. Fixture Search Backend

The fixture backend exists to make M6 deterministic before live search is used
as an acceptance gate.

Responsibilities:

- return fixed search results for fixed queries
- return stable snippets and source metadata
- optionally return zero-result or refusal cases
- map selected URLs to deterministic fetched pages already served by the
  existing fixture HTTP surface

The fixture backend should be able to support:

- one-result cases
- no-result cases
- two-source comparison cases
- follow-up reuse cases

### 4. Live Brave Adapter

The live adapter is the real search backend for QEMU-backed M6 cases.

Responsibilities:

- read `BRAVE_API_KEY` from the host environment
- issue Brave Search API requests
- map runtime query fields to provider parameters
- normalize provider results into provider-neutral result objects
- bound response size before returning it to the guest

Provider-specific fields should remain on the host side unless they are needed
for evaluation or later debugging.

## Bridge API Shape

The first bridge API can remain small.

### Search request

```json
{
  "op": "search_web",
  "query": "latest rust async runtime benchmarks",
  "top_k": 5,
  "freshness": "month",
  "domain_allowlist": [],
  "domain_denylist": [],
  "locale": "en-US"
}
```

### Search success response

```json
{
  "ok": true,
  "query": "latest rust async runtime benchmarks",
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
```

### Search failure response

```json
{
  "ok": false,
  "error": {
    "code": "backend_unavailable",
    "message": "search backend is not configured"
  }
}
```

## Fixture Mode Design

Fixture mode should not depend on Brave or any external internet result shape.

Recommended structure:

- one fixture search backend module or service
- one table of deterministic query-to-results mappings
- one table of URL-to-page-body mappings using existing fixture HTTP serving
  patterns

Example fixture mapping:

```json
{
  "query": "karpathy recent coding agent comments",
  "results": [
    {
      "id": "r1",
      "title": "Example post A",
      "url": "http://10.0.2.2:8081/source-a",
      "snippet": "Short suggestive snippet",
      "domain": "fixture.local",
      "rank": 1
    }
  ]
}
```

This allows M6 fixture cases to prove:

- search was called
- fetch was needed
- the final answer depended on fetched content

without any live provider variance.

## Live Brave Mapping

The Brave adapter should normalize at least these fields:

- Brave title -> `title`
- Brave URL -> `url`
- Brave description/snippet -> `snippet`
- derived hostname -> `domain`
- returned order -> `rank`
- provider date field when available -> `published_at`

The runtime contract should not depend on Brave-specific response nesting.

## Policy Boundaries

The first search backend should enforce at least:

- bounded `top_k`
- bounded query length
- bounded domain allowlist and denylist lengths
- bounded response result count
- bounded snippet length

The bridge should reject requests that exceed those limits with structured
`policy_denied` responses rather than silently clipping dangerous input shapes.

## Artifact Emission Responsibilities

The bridge and harness together should emit:

- `search_results.json`
- `fetched_sources.json`
- `source_memory.json` for follow-up cases

Recommended split:

- bridge records raw normalized search responses
- harness assembles run-level artifacts after the case finishes

This keeps bridge logic simple while still preserving truthful run artifacts.

## Interaction With Existing `fetch_url`

The search backend should not absorb page fetching into `search_web`.

If Brave or another provider can return content previews, those previews should
still be treated as snippets, not fetched evidence.

The research loop should remain:

1. `search_web`
2. choose result
3. `fetch_url`
4. answer from fetched evidence

## Failure Model

The backend should distinguish at least:

- invalid tool arguments
- policy denial
- backend unavailable
- provider request failure
- provider returned zero results

These should remain visible as structured result classes, not one generic
search failure string.

## Default Recommendation

For the first M6 implementation, the recommended backend shape is:

- deterministic fixture search backend for the core suite
- Brave-backed live adapter for the live suite
- one provider-neutral `search_web` tool contract
- existing `fetch_url` reused for source retrieval
