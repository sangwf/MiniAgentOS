# M7 Harness Case Matrix Draft

## Purpose

This document defines the first Harness Engineering case surface for M7.

The goal is to validate that MiniAgentOS can manage memory and context
truthfully across longer bounded loops, instead of merely storing more text and
hoping the prompt still works.

## Acceptance Strategy

M7 should not be accepted by one impressive long prompt demo.

It should be accepted by a compact harness matrix that proves:

- the runtime tracks explicit working memory
- large tool results can be compacted truthfully
- follow-up turns reuse retained state instead of starting from scratch
- resume reconstructs bounded working state honestly
- the user and harness can inspect what the runtime currently remembers

## Fixture And Live Split

The first M7 matrix should distinguish:

- **fixture cases**
  deterministic memory state, compaction, and resume validation
- **live cases**
  real QEMU-backed runs proving the same behaviors against the true runtime

Fixture cases should establish the memory contract before live long-task
behavior becomes the milestone-completion gate.

## Case Groups

### Group A: Memory Inspection Fundamentals

These cases prove the runtime exposes working memory explicitly.

#### `m7-memory-inspection`

Goal:

- the runtime accumulates bounded working memory during one or more turns
- the memory surface can be inspected directly

Expected behaviors:

- at least one memory inspection action such as `memory_status`, `list_memory`,
  or `read_memory`
- memory artifacts are emitted

Harness assertions:

- `memory_snapshot.json` exists and is non-empty
- at least one entry has a recognized `kind`
- `context_snapshot.json` shows a distinct memory layer

Suggested fixture:

- a deterministic two-turn conversation that stores one source fact and one
  task fact

#### `m7-context-budget-report`

Goal:

- the runtime exposes prompt-budget usage by layer

Expected behaviors:

- one memory inspection action

Harness assertions:

- `context_budget.json` or `context_snapshot.json` reports the sizes of:
  - instructions
  - current request
  - latest tool result
  - working memory
  - recent conversation

Suggested fixture:

- one turn with a medium-sized tool result so budget accounting is non-trivial

### Group B: Truthful Compaction

These cases prove the runtime compacts context honestly instead of silently
dropping it.

#### `m7-followup-after-large-tool-result`

Goal:

- turn 1 produces a large tool result
- runtime compacts it into working memory
- turn 2 asks a follow-up answerable from the retained compacted state

Expected behaviors:

- turn 1 records the large result
- turn 2 answers without needing the full raw blob again

Harness assertions:

- `memory_events.json` shows compaction or bounded retention
- `memory_snapshot.json` marks compacted entries as `state: compacted`
- follow-up answer remains correct

Suggested fixture:

- a synthetic large source body or execution output with one key fact buried in
  the middle

#### `m7-truthful-compaction`

Goal:

- runtime compacts a large memory entry
- later inspection shows what was retained and that the entry was compacted

Expected behaviors:

- one compaction event
- one later inspection event

Harness assertions:

- compacted entry still includes provenance
- dropped detail is not falsely presented as retained raw state
- harness can see `raw -> compacted` transition

Suggested fixture:

- a large fetched page summarized into a bounded retained fact set

### Group C: Research Memory

These cases prove research evidence survives into bounded follow-up state.

#### `m7-research-memory-followup`

Goal:

- turn 1 performs research and fetches evidence
- runtime stores retained source memory
- turn 2 asks a narrow follow-up based on that research

Expected behaviors:

- turn 1 stores source-related memory
- turn 2 answers from retained evidence, with or without additional fetches

Harness assertions:

- `memory_snapshot.json` contains at least one `source` entry
- `context_snapshot.json` includes `Known sources` or equivalent retained layer
- follow-up answer remains consistent with the earlier fetched evidence

Suggested fixture:

- first turn asks for an event summary
- second turn asks for only one subclaim or one sourced quote summary

#### `m7-research-memory-reset-boundary`

Goal:

- runtime completes one research thread
- a later unrelated thread should not inherit stale research state as if it
  were still current

Expected behaviors:

- old source memory is either absent, demoted, or clearly separated

Harness assertions:

- unrelated follow-up does not answer from stale sources without saying so
- memory inspection shows bounded retention instead of uncontrolled growth

Suggested fixture:

- two unrelated search tasks in one session

### Group D: Coding Memory

These cases prove bounded coding state can persist across interruption and
follow-up.

#### `m7-coding-memory-followup`

Goal:

- turn 1 performs a bounded coding loop and records the result
- turn 2 asks a follow-up question about what changed or why

Expected behaviors:

- workspace- or execution-related memory is retained
- follow-up explanation stays consistent with the actual prior edit/run result

Harness assertions:

- `memory_snapshot.json` contains `workspace` or `execution` entries
- follow-up answer reflects earlier coding state, not invention

Suggested fixture:

- first turn fixes a small bug
- second turn asks "what was the bug?" or "why did you change that line?"

#### `m7-resume-interrupted-task`

Goal:

- a bounded coding or research task is interrupted after useful work
- runtime resumes from a checkpoint instead of restarting from zero

Expected behaviors:

- one checkpoint save
- one checkpoint resume

Harness assertions:

- `checkpoint_snapshot.json` exists
- resumed turn sees the expected retained entries
- final answer or completion state reflects continuity, not restart

Suggested fixture:

- turn 1 gathers sources or reads workspace files
- session stops
- resumed session finishes the answer or repair

### Group E: Honest Failure And Boundedness

These cases prove M7 stays policy-controlled.

#### `m7-refuse-oversized-memory-write`

Goal:

- runtime is asked to retain or expose more memory than policy allows

Expected behaviors:

- structured refusal or bounded truncation

Harness assertions:

- policy denial is explicit
- runtime does not crash
- emitted artifacts remain internally consistent

Suggested fixture:

- one oversized synthetic memory entry request or compaction target

#### `m7-resume-missing-checkpoint`

Goal:

- runtime is asked to resume an unknown checkpoint

Expected behaviors:

- structured failure

Harness assertions:

- error is explicit and trace-visible
- no fabricated restored state appears in memory artifacts

Suggested fixture:

- unknown checkpoint ID supplied to resume path

## Recommended First Acceptance Bar

The first M7 implementation should require these fixture cases:

- `m7-memory-inspection`
- `m7-context-budget-report`
- `m7-followup-after-large-tool-result`
- `m7-truthful-compaction`
- `m7-research-memory-followup`
- `m7-coding-memory-followup`
- `m7-resume-interrupted-task`

The first live M7 bar should then require at least:

- one QEMU-backed long research follow-up using retained memory
- one QEMU-backed coding follow-up using retained memory
- one QEMU-backed resume case with inspectable memory artifacts

## Evaluation Priorities

M7 should not be evaluated only on whether the final answer is still correct.

The most important evaluator questions are:

- what did the runtime retain?
- what did it compact?
- what did it drop?
- can the user inspect that state?
- did the resumed task truly continue from persisted memory?

That is the difference between "more prompt text" and a real memory substrate.
