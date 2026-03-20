# M7 Artifact Contract Draft

## Purpose

This document defines the first artifact contract for M7 Harness Engineering.

The goal is to make M7 evaluation deterministic by fixing:

- which memory-related artifacts every M7 run must produce
- what each artifact means
- which fields the evaluator can rely on

without depending on manual prompt inspection alone.

## Design Principles

- artifact names should remain stable across fixture and live M7 runs
- artifacts should distinguish:
  - runtime memory state
  - prompt assembly state
  - memory changes over time
  - persisted checkpoint state
- the first contract should stay compact and evaluator-friendly
- artifacts should reflect truthful retention and compaction, not only final
  answer correctness

## Required Artifacts

Every M7 case should preserve at least:

- `memory_snapshot.json`
- `memory_events.json`
- `context_snapshot.json`
- `context_budget.json`
- `tool_calls.json`
- `session_transcript.json`
- `report.json`

For resume-oriented cases, the first draft should also preserve:

- `checkpoint_snapshot.json`

## Artifact Definitions

### `memory_snapshot.json`

Purpose:

- record the retained working-memory state visible to the runtime
- let the evaluator inspect what the runtime claims to remember

Suggested top-level shape:

```json
{
  "entries": [
    {
      "id": "mem-12",
      "kind": "source",
      "summary": "Fetched Brave documentation describing X-Subscription-Token",
      "source": "fetch_url:https://brave.com/search/api/",
      "state": "raw",
      "created_turn": 2,
      "updated_turn": 2
    },
    {
      "id": "mem-13",
      "kind": "task",
      "summary": "User wants the Brave authentication header",
      "source": "user_turn",
      "state": "derived",
      "created_turn": 1,
      "updated_turn": 1
    }
  ]
}
```

Minimum required fields per entry:

- `id`
- `kind`
- `summary`
- `source`
- `state`

Optional but strongly recommended:

- `created_turn`
- `updated_turn`
- `chars`
- `estimated_tokens`

### `memory_events.json`

Purpose:

- record memory mutations across the run
- make compaction, insertion, update, and removal trace-visible

Suggested top-level shape:

```json
{
  "events": [
    {
      "turn_index": 0,
      "event": "memory_added",
      "entry_id": "mem-13",
      "kind": "task"
    },
    {
      "turn_index": 1,
      "event": "memory_compacted",
      "entry_id": "mem-12",
      "from_state": "raw",
      "to_state": "compacted"
    }
  ]
}
```

Minimum required fields per event:

- `turn_index`
- `event`
- `entry_id`

Optional but strongly recommended:

- `kind`
- `from_state`
- `to_state`
- `reason`

### `context_snapshot.json`

Purpose:

- record the runtime-owned prompt assembly shape for each model turn
- prove which sections were actually included

Suggested top-level shape:

```json
{
  "turns": [
    {
      "turn_index": 1,
      "sections": [
        {
          "name": "Current request",
          "chars": 48
        },
        {
          "name": "Latest tool result",
          "chars": 320
        },
        {
          "name": "Working memory",
          "chars": 612
        },
        {
          "name": "Recent conversation",
          "chars": 188
        }
      ]
    }
  ]
}
```

Minimum required fields per turn:

- `turn_index`
- `sections`

Minimum required fields per section:

- `name`
- `chars`

Optional but useful:

- `truncated`
- `entry_ids`

### `context_budget.json`

Purpose:

- expose prompt-budget usage by layer
- support evaluator checks around bounded compaction and context control

Suggested top-level shape:

```json
{
  "turns": [
    {
      "turn_index": 1,
      "instructions_chars": 1820,
      "current_request_chars": 64,
      "latest_tool_result_chars": 910,
      "working_memory_chars": 1220,
      "recent_conversation_chars": 401,
      "estimated_total_tokens": 1120
    }
  ]
}
```

Minimum required fields per turn:

- `turn_index`
- `estimated_total_tokens`

The remaining per-layer budget fields should be treated as strongly recommended
for the first M7 implementation.

### `tool_calls.json`

Purpose:

- preserve ordered tool activity for evaluator checks
- correlate memory mutations with external events

This file already exists in earlier milestones and should be extended, not
replaced.

M7-specific expectations:

- memory-related tool calls are recorded when present
- coding/research tool calls remain present so the evaluator can connect memory
  entries back to the underlying work

### `session_transcript.json`

Purpose:

- preserve per-turn user and assistant interaction state
- anchor memory events to actual conversation turns

This file already exists in earlier milestones and should remain the canonical
turn-level transcript.

### `report.json`

Purpose:

- preserve final evaluator pass/fail state
- summarize the key memory behaviors checked for that case

The first M7 report can remain compact, but it should expose enough metadata to
answer:

- was explicit memory present
- was compaction required
- was resume required
- did the evaluator accept the run

## Resume Artifact

### `checkpoint_snapshot.json`

Purpose:

- make persisted resume state visible to the harness
- show what was saved and what was restored

Suggested top-level shape:

```json
{
  "checkpoints": [
    {
      "checkpoint_id": "ckpt-7",
      "label": "tariff-research-thread",
      "entries": [
        "mem-12",
        "mem-13"
      ],
      "saved_turn": 3,
      "resumed_turn": 4
    }
  ]
}
```

Minimum required fields:

- `checkpoint_id`
- `entries`
- `saved_turn`

For resume cases, `resumed_turn` should also be required.

## Optional Later Artifacts

The first M7 slice does not require these, but the contract should leave room
for them:

- `memory_diff.json`
- `compaction_trace.json`
- `checkpoint_payloads/<checkpoint_id>.json`
- `turn_context/<turn_index>.txt`

These may help debugging, but they should not be required for the first M7
harness bar.

## Evaluator Assumptions

The first M7 evaluator should be able to answer these questions from artifacts
alone:

1. Did the runtime retain explicit memory entries?
2. Did those entries have the expected kinds and states?
3. Did context assembly include a distinct memory layer?
4. Did compaction happen when required?
5. Was retained state reused in follow-up turns?
6. For resume cases, was checkpoint state restored truthfully?

## Fixture And Live Consistency

The same artifact names and broad field meanings should be used for:

- fixture-backed M7 cases
- real QEMU-backed live M7 cases

Live runs may add extra fields, but they should not remove the minimum required
fields that fixture evaluation depends on.

## First-Draft Contract Summary

The first M7 harness should be considered artifact-complete when it can
reliably emit:

- `memory_snapshot.json`
- `memory_events.json`
- `context_snapshot.json`
- `context_budget.json`
- `tool_calls.json`
- `session_transcript.json`
- `report.json`

and, for resume cases:

- `checkpoint_snapshot.json`
