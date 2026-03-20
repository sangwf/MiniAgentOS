# M6 Tool Contract Draft

## Purpose

This document defines the first bounded research tool surface for M6.

The goal is to make the runtime capable of a real:

`search -> inspect results -> fetch sources -> compare -> summarize -> follow-up`

loop without requiring a full browser, unrestricted crawling, or guest-native
search indexing.

## Design Principles

- tools are runtime-owned contracts exposed inside the MiniAgentOS guest
- first implementations may be host-backed
- all tool calls must remain policy-bounded and trace-visible
- tool arguments should be structured and provider-agnostic
- search and fetch must remain distinct capabilities
- tool results must preserve the boundary between:
  - search-result metadata
  - fetched source content
  - model-written synthesis

## Shared Concepts

### Search Query

Every M6 search begins with one bounded query object.

The runtime should treat a search query as structured state, not as an opaque
provider request blob.

### Search Result

A search result is a candidate source returned by `search_web`. It is not yet
treated as fetched evidence.

Every result should carry enough metadata for the runtime to reason about:

- `title`
- `url`
- `snippet`
- `domain`
- `rank`

Optional metadata may include:

- `published_at`
- `language`
- `result_type`

### Known Sources

M6 introduces the idea of a bounded source set inside the session.

Known sources may include:

- search results returned by `search_web`
- URLs later fetched with `fetch_url`
- derived source records created when the runtime reads page content

The runtime should preserve enough metadata to answer follow-up questions about:

- which sources were seen
- which sources were actually fetched
- which source text was used in a summary or comparison

### Provider Independence

The guest-visible tool contract should not expose Brave-specific field names or
raw provider response blobs.

The host bridge may use Brave Search through `BRAVE_API_KEY`, but it should
normalize provider data before returning it to the guest.

## Core Tool

### `search_web`

Perform one bounded web search and return structured candidate sources.

Arguments:

```json
{
  "query": "latest rust async runtime benchmarks",
  "top_k": 5,
  "freshness": "month",
  "domain_allowlist": [],
  "domain_denylist": [],
  "locale": "en-US"
}
```

Rules:

- `query` is required
- `top_k` is bounded, for example `1..10`
- `freshness` is optional and normalized to a small runtime vocabulary such as:
  - `day`
  - `week`
  - `month`
  - `year`
- domain allow/deny lists are optional and bounded in length
- locale is optional and normalized by the runtime
- raw provider-specific query flags should not be part of the guest contract

Result:

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
    },
    {
      "id": "r2",
      "title": "Rust runtime performance notes",
      "url": "https://blog.example.org/post",
      "snippet": "Recent measurements show ...",
      "domain": "blog.example.org",
      "rank": 2
    }
  ],
  "truncated": false,
  "provider": "normalized"
}
```

Failure result example:

```json
{
  "ok": false,
  "error": {
    "code": "backend_unavailable",
    "message": "search backend is not configured"
  }
}
```

Policy failure example:

```json
{
  "ok": false,
  "error": {
    "code": "policy_denied",
    "message": "domain filter exceeds policy limit"
  }
}
```

## Existing Tool Interaction

### `fetch_url`

M6 does not replace `fetch_url`.

Instead, M6 relies on this separation:

- `search_web` finds candidate sources
- `fetch_url` reads the selected source

The fetched result should remain visibly different from a search result snippet.

At minimum, a fetched source should preserve:

- the source URL
- the associated search result ID when available
- the fetched body or extracted text
- whether the content was truncated

### Session Memory

M6 depends on the existing session loop from M4.

The runtime should add a bounded research-oriented source memory layer to that
session state so follow-up turns can reuse prior search and fetch results.

## Optional Tool Family

These tools are not required for the first M6 delivery, but the contract should
leave room for them:

- `list_sources`
- `read_source`
- `clear_sources`

### `list_sources`

List the currently known sources in bounded session memory.

Possible result shape:

```json
{
  "ok": true,
  "sources": [
    {
      "id": "r1",
      "url": "https://example.com/benchmarks",
      "domain": "example.com",
      "fetched": true
    }
  ]
}
```

### `read_source`

Read one source record already tracked in session memory.

This would allow the model to inspect source metadata without re-fetching.

### `clear_sources`

Explicitly clear the bounded research source set when the runtime or user wants
to start a fresh research thread.

## Trace Requirements

M6 tool calls should emit enough structured trace for the harness to recover:

- the original search query
- the returned search results
- the URLs actually fetched
- the source IDs reused in follow-up turns

The runtime should avoid flattening all research evidence into one opaque
conversation blob.

## Artifact Expectations

The first M6 harness draft should be able to extract at least:

- `search_results.json`
- `fetched_sources.json`
- `tool_calls.json`
- `session_transcript.json`

Optional later artifacts:

- `source_memory.json`
- `research_notes.json`

## Honest Research Requirement

M6 should not let the model present search-result snippets as if they were
fetched evidence.

The contract is only truthful if the runtime keeps these states visibly
distinct:

- searched
- selected
- fetched
- synthesized

## Default Live Backend Assumption

The first live M6 path should use Brave Search through `BRAVE_API_KEY`, but the
tool contract should remain provider-neutral.

The host bridge should be responsible for:

- loading `BRAVE_API_KEY`
- issuing provider requests
- normalizing provider responses
- enforcing provider-specific safety and budget limits
