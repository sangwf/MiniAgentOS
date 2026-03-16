# MiniAgentOS source document

MiniAgentOS is an agent-first runtime built around structured goals, native
skills, and observable execution traces. The system is not trying to reproduce
traditional operating system abstractions first. Instead, it focuses on a
single actionable loop: receive a goal, plan, call skills, and complete a
useful action under harness evaluation.

Harness engineering matters because the runtime should be measurable from the
start. Each run needs a prompt, a task input, a trace stream, and a real
external effect that an evaluator can check.
