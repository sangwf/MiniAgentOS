# M7 Memory Backend And Persistence Draft

## Purpose

This document defines the first implementable backend shape for M7 memory.

The goal is to realize the M7 durable memory substrate using:

- runtime-owned in-memory working state inside the guest
- bounded prompt assembly from that working state
- host-visible artifacts for inspection and harness evaluation
- optional persisted checkpoints for truthful resume

The key design rule is:

**working memory lives in runtime memory first; files are checkpoint and
artifact outputs, not the primary memory model.**

## High-Level Model

The first M7 implementation should split responsibility into three layers:

- guest working-memory plane
- guest context-assembly plane
- host persistence and artifact plane

### Guest Working-Memory Plane

Lives inside MiniAgentOS and owns:

- runtime memory entries
- memory classification
- bounded retention policy
- compaction decisions
- resume reconstruction after checkpoint import

### Guest Context-Assembly Plane

Also lives inside MiniAgentOS and owns:

- mapping runtime state into prompt sections
- budgeting context by section
- deciding which retained entries are included in the next model turn

### Host Persistence And Artifact Plane

Lives outside the guest and owns:

- run-level artifact capture
- checkpoint serialization when requested
- deterministic fixture memory backends
- harness inspection and evaluation

The guest should not depend on a host file system to keep its current working
memory alive. The host exists for observability and bounded persistence, not as
the primary source of truth for active runtime state.

## Main Components

### 1. Guest Memory Manager

This is the core M7 subsystem.

Responsibilities:

- store working-memory entries in RAM
- assign memory IDs
- classify entries by kind
- update entry state (`raw`, `compacted`, `derived`)
- expose inspection surfaces such as `list_memory`, `read_memory`, and
  `memory_status`

The first implementation can be simple:

- fixed-capacity arrays
- bounded summary fields
- bounded detail buffers
- runtime-governed eviction or compaction

It does not need a general database or guest-native filesystem.

### 2. Context Assembler

The context assembler maps runtime state into prompt sections.

Responsibilities:

- keep `Current request` authoritative
- decide how much of `Latest tool result` is raw vs compacted
- include `Working memory` explicitly
- include `Known sources` explicitly when present
- include bounded `Recent conversation`
- compute prompt-layer budgets and expose them to artifacts

This layer is where M7 materially improves on M4/M6: prompt construction stops
being "whatever recent text still fits" and becomes a runtime-owned policy.

### 3. Compaction Engine

The first M7 compaction path can stay simple and bounded.

Responsibilities:

- detect when raw tool output is too large to keep forwarding directly
- produce bounded retained summaries
- preserve provenance and explicit uncertainty
- mark entries as `compacted`

The compaction engine should not claim to preserve detail that was dropped.

Recommended first-wave behavior:

- compact large fetched sources into retained evidence summaries
- compact large execution outputs into retained failure/success summaries
- leave small task or source entries unmodified

### 4. Checkpoint Manager

The checkpoint manager is responsible for durable resume.

Responsibilities:

- serialize bounded memory state when a checkpoint is requested
- restore that state into guest memory on resume
- keep checkpoint metadata explicit and inspectable

The first checkpoint format can be small and direct:

- checkpoint ID
- saved turn index
- retained memory entries
- optional budget summary

The checkpoint should be a snapshot of runtime-owned memory state, not an
opaque replay transcript.

### 5. Fixture Memory Backend

The fixture backend exists to make M7 deterministic before live memory behavior
is used as an acceptance gate.

Responsibilities:

- return fixed memory snapshots for fixed case setups
- simulate compaction transitions deterministically
- simulate checkpoint save and resume deterministically
- support evaluator checks for memory truthfulness and continuity

This backend should be able to support:

- inspection-only cases
- compaction cases
- follow-up reuse cases
- resume cases

### 6. Live Persistence Adapter

The live persistence adapter is the host-side layer for checkpoint storage and
artifact emission.

Responsibilities:

- receive or reconstruct memory snapshots from runtime trace
- persist checkpoint payloads in a stable host-visible format
- keep fixture/live artifact shapes aligned

The first live implementation may be very simple:

- JSON checkpoint files under `output/`
- one current checkpoint payload per case or manual session

That is enough for M7. It does not need a general storage service.

## Recommended Data Flow

The first M7 data flow should look like this:

1. user turn arrives
2. runtime creates or updates task memory
3. tool call happens
4. runtime creates or updates source/workspace/execution memory
5. compaction runs when bounded policy requires it
6. context assembler builds the next model prompt from explicit layers
7. trace/artifact hooks expose memory and context state to the host
8. optional checkpoint export persists bounded memory state for resume

The important point is that checkpoint export happens **after** the guest has
already created its own memory state. Persistence is downstream of memory, not
the other way around.

## Suggested Guest Data Model

The first implementation can stay static and bounded.

Recommended conceptual structures:

- `MemoryEntry`
  - `id`
  - `kind`
  - `state`
  - `summary`
  - `detail`
  - `source`
  - `created_turn`
  - `updated_turn`
- `MemoryStore`
  - fixed-capacity entry table
  - lookup by ID
  - bounded counts by kind
- `ContextBudget`
  - instruction chars
  - current request chars
  - latest tool result chars
  - working memory chars
  - recent conversation chars
  - estimated total tokens
- `CheckpointRecord`
  - checkpoint ID
  - saved turn
  - entry references or serialized entries

The exact Rust shape can evolve, but the semantics should match the M7 tool and
artifact contracts.

## Policy Boundaries

The first M7 backend should enforce at least:

- maximum entry count
- maximum per-entry summary/detail length
- maximum checkpoint size
- maximum number of active checkpoints
- maximum compaction output length

The runtime should react to overflow through:

- compaction
- bounded eviction
- explicit truncation markers
- structured policy errors when necessary

not by silently corrupting memory state.

## Interaction With Existing Session State

M7 should not delete the current M4/M5/M6 session machinery immediately.

Instead:

- `Recent conversation` can continue to come from session history
- `Session state` can continue to come from the existing bounded KV layer
- M7 adds explicit working memory alongside them

This keeps the milestone additive and makes migration practical.

## Trace And Artifact Emission

The backend should emit enough structured information for the host to recover:

- current memory snapshot
- memory mutations
- prompt assembly layout
- prompt-layer budgets
- checkpoint save and resume events

The host should not need to infer memory behavior only from final prompt text.

## Failure Model

The backend should distinguish at least:

- memory entry overflow
- compaction failure
- checkpoint save failure
- checkpoint restore failure
- unknown memory ID
- unknown checkpoint ID

These should become structured runtime-visible failures, not silent corruption.

## First Implementable Cut

The first M7 implementation should aim for this narrow, realistic slice:

- in-memory guest `MemoryStore`
- explicit `Working memory` prompt section
- `list_memory`, `read_memory`, `memory_status`
- artifact emission for:
  - `memory_snapshot.json`
  - `memory_events.json`
  - `context_snapshot.json`
  - `context_budget.json`
- one bounded checkpoint export/import path

That is enough to make M7 real without turning it into a full storage or
filesystem milestone.
