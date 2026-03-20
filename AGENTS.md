# AGENTS.md

## Goal

Build MiniAgentOS as an agent-first runtime, not as a traditional general
purpose operating system. The harness is a first-class part of the system.

## Project Map

- `README.md`: repository overview and quick start
- `docs/milestones/m0.md`: current Milestone 0 definition
- `docs/milestones/m1.md`: current Milestone 1 definition
- `docs/milestones/m2.md`: current Milestone 2 definition
- `docs/milestones/m3.md`: current Milestone 3 definition
- `docs/milestones/m4.md`: current Milestone 4 definition
- `docs/milestones/m5.md`: current Milestone 5 definition
- `docs/milestones/m6.md`: current Milestone 6 definition
- `docs/milestones/m7.md`: current Milestone 7 definition
- `docs/schemas/task.schema.json`: task file reference schema
- `docs/schemas/trace-event.schema.json`: trace event reference schema
- `docs/schemas/intent-ir.schema.json`: intent artifact reference schema
- `harness/config.example.json`: example config for a real QEMU-backed runtime
- `harness/config.fixture.json`: config used to validate the harness itself
- `harness/config.openai.json`: real-model config using `OPENAI_API_KEY` and `gpt-5.4-mini`
- `harness/config.runtime-m5.json`: real-runtime M5 config using QEMU, the host M5 bridge, and a live OpenAI path
- `harness/cases/`: reusable harness cases
- `harness/fixtures/fake_agent.py`: host-side fixture agent for harness self-test
- `harness/lib/http_fixtures.py`: source, interpretation, model, and sink HTTP fixtures
- `harness/lib/run_case.py`: main case runner
- `harness/lib/evaluator.py`: harness evaluator
- `runtime/`: the real MiniOS codebase being wired into the harness
- `tools/agent_run.py`: one-command manual launcher for the live agent path
- `tools/m5_run.py`: compatibility wrapper for the older manual launcher name
- `tools/view_llm_log.py`: viewer for readable `llm_api_log.jsonl` rendering
- `scripts/check.py`: structural repository checks
- `bin/check`: single command for structural checks
- `bin/qemu-system-aarch64-local`: wrapper for the local bottle-based QEMU fallback
- `bin/setup-qemu-local`: extract the local QEMU fallback from cached bottles
- `bin/setup-toolchain`: install Rust, rust-src, and QEMU prerequisites
- `bin/run-case`: run one case
- `bin/run-suite`: run the default M1 suite
- `bin/validate`: validate the harness with the fixture agent

## Commands

- `./bin/check`: verify repository layout and case/config integrity
- `./bin/setup-toolchain`: install or update the local toolchain expected by the harness
- `./bin/run-case <case> --config <config>`: run one case against an agent command
- `./bin/run-suite --suite m1 --config <config>`: run the M1 suite
- `./bin/run-suite --suite m2 --config <config>`: run the M2 suite
- `./bin/run-suite --suite m3 --config <config>`: run the M3 suite surface
- `./bin/run-suite --suite m4 --config <config>`: run the initial M4 harness surface
- `./bin/run-suite --suite m5 --config <config>`: run the M5 fixture suite
- `./bin/run-suite --suite m5live --config <config>`: run the QEMU-backed live M5 suite
- `./bin/run-suite --suite m6 --config <config>`: run the M6 fixture suite
- `./bin/run-suite --suite m6live --config <config>`: run the QEMU-backed live M6 suite
- `./bin/run-suite --suite m7 --config <config>`: run the M7 fixture suite
- `./bin/run-suite --suite m7live --config <config>`: run the QEMU-backed live M7 suite
- `./bin/validate`: run the default M0 case against the fixture agent

## Harness Rules

- Treat the harness contract as stable unless the spec changes:
  prompt line, JSON task input, `TRACE ` lines, and sink POST are all part of
  the public interface.
- Keep the runtime launch environment reproducible. Prefer wiring required PATH
  prefixes through harness config and launch wrappers instead of assuming the
  user's shell startup files are already correct.
- Keep the host-side harness dependency-light. Use Python stdlib unless a clear
  need appears.
- Prefer controlled fixture services over internet dependencies for automated
  validation.
- Favor deterministic evaluation criteria over vague qualitative checks.
- Preserve the output artifact structure under `output/` so regressions stay
  diffable.
- When changing M3 behavior, prefer emitting enough structured trace for the
  harness to extract `intent_ir.json` and validate compiled intent directly.
- When changing M4 behavior, prefer emitting enough structured trace for the
  harness to extract `tool_calls.json` and `session_transcript.json` directly.

## M0 Definition Of Done

- A case runner can start the runtime and observe a boot prompt.
- A structured task can be submitted over stdin.
- The runtime emits ordered trace events.
- The runtime performs a real fetch from the source fixture and a real POST to
  the result sink.
- The evaluator can mark the run pass or fail without manual inspection.

## M1 Direction

- Move planning from one fixed skill chain to a small governed skill runtime.
- Treat skills as planner-visible and policy-controlled actions.
- Treat tools as internal reusable capabilities that skills compose.
- Keep a deterministic mock gateway for regression.
- Require a live OpenAI-backed path using `OPENAI_API_KEY` and `gpt-5.4-mini`.
- Add refusal/failure harness coverage without weakening the live-model bar.

## M2 Direction

- Make `Goal >` a real human-facing goal shell instead of a debug command prompt.
- Preserve JSON task input as the stable automation and harness contract.
- Compile supported natural-language goals locally inside the runtime.
- Support a local `fetch -> summarize` path that returns the summary directly to
  UART without requiring sink/model services.
- Keep goal compilation visible in trace and evaluation artifacts.
- Do not let the official M2 bar depend on a host-side translation service.

## M3 Direction

- Make free-form natural language the real primary interface at `Goal >`.
- Add a model-driven agent core instead of relying on narrow goal templates as
  the main path.
- Compile natural-language requests into explicit Intent IR before capability
  execution.
- Keep user-facing constraints such as language, style, and output count as
  structured intent fields, not as prompt-only hints.
- Remove the requirement for a host-side interpretation service on the default
  manual path.
- Remove the requirement for a host-side OpenAI gateway on the official live
  path.
- Allow a plain host-side transport proxy for outbound connectivity when
  required, but keep it dumb. The current runtime expects `10.0.2.2:7897`.
- Keep capability selection bounded, policy-controlled, and trace-visible.
- Return default task results directly to UART instead of requiring sink-based
  completion for the primary manual path.
- Keep the harness as evaluator, fixture provider, and optional secret-injection
  helper, not as a replacement for native goal understanding or native provider
  access.

## M4 Direction

- Replace M3's action-branch-driven execution with a standard sessioned
  `model -> tools -> model` loop.
- Build M4 model input from explicit sections instead of one raw conversation
  blob.
- Treat the latest user request as a first-class, authoritative prompt section.
- Keep the M4 prompt contract explicit and layered:
  - `Current request`
  - `Latest tool result`
  - `Session state`
  - `Recent conversation`
- Keep recent conversation as a suffix-preserved context layer, not the only
  place where the current request lives.
- Record failed turns back into session history so the next turn starts from a
  truthful session state.
- Treat `call_model` as part of the loop machinery, not as a planner-visible
  tool for the primary manual path.
- Keep the exposed tool surface intentionally small, explicit, and honest.
- Include X/Twitter read and write capabilities in the first M4 tool set.
- Add session history and follow-up turns as first-class runtime behavior.
- Keep tool policy, host policy, and loop budgets enforced at runtime.
- Make harness evaluation care about loop behavior and follow-up turns, not
  only one-shot task completion.
- Reuse the guest-side OpenAI transport across consecutive OpenAI-only turns
  when possible; fall back to reconnects when the transport is no longer valid.

## M5 Direction

Status: implemented in its first bounded form. The runtime now has a bounded
workspace/edit/run substrate, an M5 fixture suite, and a QEMU-backed live M5
suite. Further expansion is still possible, but M5 is no longer planned-only.

- Add a bounded workspace abstraction.
- Add real file inspection and editing tools.
- Add a real bounded execution primitive and observable output.
- Use those capabilities to enable the first real inspect/edit/run/observe
  coding loops.
- Keep execution and workspace access tightly policy-controlled.

## M6 Direction

Status: partially implemented. M6 now has a fixture-backed research suite and a
first QEMU-backed live search slice, but follow-up-heavy live research coverage
is still incomplete.

- Add a bounded web search primitive.
- Keep search and fetch as distinct runtime capabilities.
- Introduce a bounded source-set model for follow-up research turns.
- Use `BRAVE_API_KEY` as the default live search backend secret.
- Validate search, source selection, multi-source synthesis, and research
  follow-up behavior in the harness.

## M7 Direction

Status: partially implemented. M7 now has a first fixture-backed memory suite,
first QEMU-backed live inspection and truthful-compaction slices, artifact
contract, evaluator surface, and first in-guest memory inspection and
truthful-compaction slices; the broader guest-side explicit memory runtime is
still ahead.

- Make memory and context a first-class runtime substrate instead of a side
  effect of concatenating prior turns.
- Introduce explicit working-memory classes for task, source, workspace, and
  execution state.
- Add truthful context compaction so large tool results and long histories do
  not dominate later turns.
- Add inspectable memory state so the user and harness can see what the runtime
  currently remembers and why.
- Add durable resume behavior for interrupted long-running coding and research
  tasks.
- Validate memory truthfulness, compaction, and resume behavior in the harness.
