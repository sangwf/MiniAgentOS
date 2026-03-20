# M7 Tool Contract Draft

## Purpose

This document defines the first bounded memory and context tool surface for M7.

The goal is not to expose a large free-form memory API immediately. The goal is
to give MiniAgentOS the minimum truthful contract needed to:

- inspect current working memory
- understand how prompt context was assembled
- compact large context safely
- resume work from persisted state

## Design Principles

- memory is runtime-owned state, not an ungoverned model scratchpad
- memory entries must remain inspectable and attributable
- memory compaction must be truthful and bounded
- memory tools should reveal state before they allow broad mutation
- the first implementation may be host-backed, but the guest-visible contract
  must remain stable
- memory should stay distinct from:
  - raw recent conversation
  - raw tool output
  - provider-side hidden memory

## Shared Concepts

### Memory Entry

A memory entry is one bounded retained unit of working state.

Every entry should carry enough metadata for the runtime and harness to answer:

- what this memory is about
- what class of memory it belongs to
- where it came from
- whether it is raw, compacted, or derived

Suggested minimum fields:

- `id`
- `kind`
- `summary`
- `source`
- `state`

Optional fields:

- `created_turn`
- `updated_turn`
- `chars`
- `estimated_tokens`
- `provenance`

### Memory Kinds

The first M7 contract should distinguish at least:

- `task`
- `source`
- `workspace`
- `execution`
- `conversation`

This is important because the runtime should not flatten "what the user asked",
"what a fetched page said", and "what a test run reported" into one opaque
bucket.

### Memory State

Each entry should indicate whether it is:

- `raw`
- `compacted`
- `derived`

This lets the runtime and harness reason about whether a later answer came from
full evidence, a compacted memory, or a derived note.

### Context Snapshot

A context snapshot is the runtime-owned description of what was actually carried
into a model turn.

The M7 contract should allow inspection of the prompt as assembled from:

- instructions
- current request
- latest tool result
- working memory
- known sources
- session state
- recent conversation

This does not require exposing provider-specific raw wire formats to the model.
It does require giving the user and harness an honest view of the runtime's own
context assembly.

### Resume Checkpoint

A resume checkpoint is a bounded persisted representation of session state and
working memory that can be used to reconstruct a truthful working state after
interruption.

The checkpoint should not be a magical hidden transcript. It should be a
runtime-owned artifact with explicit contents and limits.

## Default Guest Surface

The first M7 implementation does not need a large new planner-visible tool set.

It should expose a small inspection-first surface. The recommended minimum
surface is:

- `list_memory`
- `read_memory`
- `memory_status`

Optional first-wave additions:

- `compact_memory`
- `save_checkpoint`
- `resume_checkpoint`

Shell commands such as `memory-status` and `memory-dump` may exist in parallel,
but the tool contract should be the main stable surface for the session loop
and harness.

## Core Tools

### `list_memory`

List the currently retained working-memory entries in bounded form.

Arguments:

```json
{
  "kind": "source",
  "limit": 20
}
```

Rules:

- both fields are optional
- `kind` must be normalized to one of the known memory classes
- `limit` is bounded, for example `1..100`

Result:

```json
{
  "ok": true,
  "entries": [
    {
      "id": "mem-12",
      "kind": "source",
      "summary": "Brave docs page describing the search authentication header",
      "source": "fetch_url:https://brave.com/search/api/",
      "state": "raw"
    },
    {
      "id": "mem-13",
      "kind": "task",
      "summary": "User wants a sourced answer about Brave authentication headers",
      "source": "user_turn",
      "state": "derived"
    }
  ],
  "truncated": false
}
```

### `read_memory`

Read one specific memory entry in more detail.

Arguments:

```json
{
  "id": "mem-12"
}
```

Result:

```json
{
  "ok": true,
  "entry": {
    "id": "mem-12",
    "kind": "source",
    "summary": "Brave docs page describing the search authentication header",
    "detail": "The fetched page documents the required header as X-Subscription-Token.",
    "source": "fetch_url:https://brave.com/search/api/",
    "state": "raw",
    "created_turn": 2,
    "updated_turn": 2
  }
}
```

Failure example:

```json
{
  "ok": false,
  "error": {
    "code": "unknown_memory_id",
    "message": "memory entry was not found"
  }
}
```

### `memory_status`

Return bounded summary statistics about current context and memory usage.

Arguments:

```json
{}
```

Result:

```json
{
  "ok": true,
  "counts": {
    "task": 1,
    "source": 3,
    "workspace": 0,
    "execution": 0,
    "conversation": 4
  },
  "budget": {
    "instructions_chars": 1820,
    "current_request_chars": 64,
    "latest_tool_result_chars": 910,
    "working_memory_chars": 1220,
    "recent_conversation_chars": 401,
    "estimated_total_tokens": 1120
  },
  "checkpoint_available": true
}
```

The important thing here is not exact token accounting. The important thing is
that the runtime can expose how much prompt budget each layer is consuming.

## Optional Tools

### `compact_memory`

Compact one or more memory entries under runtime policy.

Arguments:

```json
{
  "ids": ["mem-12", "mem-13"],
  "mode": "bounded_summary"
}
```

Rules:

- compaction should remain runtime-governed
- the model should not be allowed to rewrite provenance arbitrarily
- a compacted entry must remain visibly compacted

Result:

```json
{
  "ok": true,
  "updated": [
    {
      "id": "mem-12",
      "state": "compacted"
    }
  ]
}
```

### `save_checkpoint`

Persist the current bounded memory state for later resume.

Arguments:

```json
{
  "label": "tariff-research-thread"
}
```

Result:

```json
{
  "ok": true,
  "checkpoint_id": "ckpt-7",
  "entries": 6
}
```

### `resume_checkpoint`

Resume from a previously persisted checkpoint.

Arguments:

```json
{
  "checkpoint_id": "ckpt-7"
}
```

Result:

```json
{
  "ok": true,
  "checkpoint_id": "ckpt-7",
  "restored_entries": 6
}
```

## Prompt Assembly Interaction

M7 should keep prompt assembly explicit.

The runtime should be able to map memory state into prompt sections such as:

- `Working memory`
- `Known sources`
- `Workspace memory`

This matters because the harness must be able to distinguish:

- raw prior tool output
- compacted memory
- recent conversation suffix

The tool contract should support inspecting those layers without pretending they
are all the same thing.

## Trace Requirements

M7 tool calls and prompt assembly should emit enough structured trace for the
harness to recover:

- what memory entries existed before a turn
- what entries changed after a turn
- whether compaction occurred
- whether a checkpoint was created or resumed
- what prompt-layer budgets were active for the turn

The runtime should avoid hiding memory behavior inside one opaque prompt blob.

## Artifact Expectations

The first M7 harness draft should be able to extract at least:

- `memory_snapshot.json`
- `memory_events.json`
- `context_snapshot.json`
- `tool_calls.json`
- `session_transcript.json`

Optional later artifacts:

- `checkpoint_snapshot.json`
- `memory_diff.json`
- `context_budget.json`

## Honest Memory Requirement

M7 must not let the runtime silently pretend it remembers details it has
already compacted away.

The system should be able to say, in artifacts and inspectable state:

- this fact was retained in raw form
- this fact was compacted
- this detail was dropped

That honesty is more important than exposing a large mutation surface early.
