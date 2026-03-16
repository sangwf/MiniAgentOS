#!/usr/bin/env python3

from __future__ import annotations

import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent

REQUIRED_FILES = [
    ROOT / "README.md",
    ROOT / "AGENTS.md",
    ROOT / "docs" / "milestones" / "m0.md",
    ROOT / "docs" / "milestones" / "m1.md",
    ROOT / "docs" / "milestones" / "m2.md",
    ROOT / "docs" / "milestones" / "m3.md",
    ROOT / "docs" / "milestones" / "m4.md",
    ROOT / "docs" / "milestones" / "m5.md",
    ROOT / "docs" / "schemas" / "task.schema.json",
    ROOT / "docs" / "schemas" / "trace-event.schema.json",
    ROOT / "docs" / "schemas" / "intent-ir.schema.json",
    ROOT / "harness" / "README.md",
    ROOT / "harness" / "config.example.json",
    ROOT / "harness" / "config.fixture.json",
    ROOT / "harness" / "config.openai.json",
    ROOT / "harness" / "cases" / "m0-fetch-summarize-post" / "task.json",
    ROOT / "harness" / "cases" / "m0-fetch-summarize-post" / "source.md",
    ROOT / "harness" / "cases" / "m1-allow-fetch-model-post" / "task.json",
    ROOT / "harness" / "cases" / "m1-allow-fetch-model-post" / "source.md",
    ROOT / "harness" / "cases" / "m1-deny-disallowed-host" / "task.json",
    ROOT / "harness" / "cases" / "m1-deny-disallowed-host" / "source.md",
    ROOT / "harness" / "cases" / "m1-deny-disallowed-skill" / "task.json",
    ROOT / "harness" / "cases" / "m1-deny-disallowed-skill" / "source.md",
    ROOT / "harness" / "cases" / "m1-model-gateway-error" / "task.json",
    ROOT / "harness" / "cases" / "m1-model-gateway-error" / "source.md",
    ROOT / "harness" / "cases" / "m2-nl-fetch-model-post" / "task.json",
    ROOT / "harness" / "cases" / "m2-nl-fetch-model-post" / "source.md",
    ROOT / "harness" / "cases" / "m2-nl-fetch-summarize" / "task.json",
    ROOT / "harness" / "cases" / "m2-nl-fetch-summarize" / "source.md",
    ROOT / "harness" / "cases" / "m2-nl-refuse-disallowed-host" / "task.json",
    ROOT / "harness" / "cases" / "m2-nl-refuse-disallowed-host" / "source.md",
    ROOT / "harness" / "cases" / "m2-nl-unsupported-goal" / "task.json",
    ROOT / "harness" / "cases" / "m2-nl-unsupported-goal" / "source.md",
    ROOT / "harness" / "cases" / "m2-nl-compilation-error" / "task.json",
    ROOT / "harness" / "cases" / "m2-nl-compilation-error" / "source.md",
    ROOT / "harness" / "cases" / "m3-nl-summarize-url" / "task.json",
    ROOT / "harness" / "cases" / "m3-nl-summarize-url" / "source.md",
    ROOT / "harness" / "cases" / "m3-nl-summarize-url-zh" / "task.json",
    ROOT / "harness" / "cases" / "m3-nl-summarize-url-zh" / "source.md",
    ROOT / "harness" / "cases" / "m3-nl-summarize-url-openai" / "task.json",
    ROOT / "harness" / "cases" / "m3-nl-summarize-url-openai" / "source.md",
    ROOT / "harness" / "cases" / "m3-nl-post-summary" / "task.json",
    ROOT / "harness" / "cases" / "m3-nl-post-summary" / "source.md",
    ROOT / "harness" / "cases" / "m3-nl-policy-refusal" / "task.json",
    ROOT / "harness" / "cases" / "m3-nl-policy-refusal" / "source.md",
    ROOT / "harness" / "cases" / "m3-nl-interpretation-error" / "task.json",
    ROOT / "harness" / "cases" / "m3-nl-interpretation-error" / "source.md",
    ROOT / "harness" / "cases" / "m3-nl-chat-refusal" / "task.json",
    ROOT / "harness" / "cases" / "m3-nl-chat-refusal" / "source.md",
    ROOT / "harness" / "cases" / "m4-loop-post-tweet" / "task.json",
    ROOT / "harness" / "cases" / "m4-loop-post-tweet" / "source.md",
    ROOT / "harness" / "cases" / "m4-loop-post-url" / "task.json",
    ROOT / "harness" / "cases" / "m4-loop-post-url" / "source.md",
    ROOT / "harness" / "cases" / "m4-loop-search-recent-posts" / "task.json",
    ROOT / "harness" / "cases" / "m4-loop-search-recent-posts" / "source.md",
    ROOT / "harness" / "cases" / "m4-loop-search-recent-posts" / "x.json",
    ROOT / "harness" / "cases" / "m4-loop-get-user-posts" / "task.json",
    ROOT / "harness" / "cases" / "m4-loop-get-user-posts" / "source.md",
    ROOT / "harness" / "cases" / "m4-loop-get-user-posts" / "x.json",
    ROOT / "harness" / "cases" / "m4-loop-followup-question" / "task.json",
    ROOT / "harness" / "cases" / "m4-loop-followup-question" / "source.md",
    ROOT / "harness" / "cases" / "m4-loop-followup-question" / "x.json",
    ROOT / "harness" / "cases" / "m4-loop-tool-denied" / "task.json",
    ROOT / "harness" / "cases" / "m4-loop-tool-denied" / "source.md",
    ROOT / "harness" / "cases" / "m4-loop-iteration-budget" / "task.json",
    ROOT / "harness" / "cases" / "m4-loop-iteration-budget" / "source.md",
    ROOT / "harness" / "cases" / "m4-loop-tools-meta" / "task.json",
    ROOT / "harness" / "cases" / "m4-loop-tools-meta" / "source.md",
    ROOT / "harness" / "fixtures" / "fake_agent.py",
    ROOT / "harness" / "lib" / "run_case.py",
    ROOT / "harness" / "lib" / "run_suite.py",
    ROOT / "harness" / "lib" / "evaluator.py",
    ROOT / "harness" / "lib" / "http_fixtures.py",
    ROOT / "bin" / "check",
    ROOT / "bin" / "qemu-system-aarch64-local",
    ROOT / "bin" / "run-case",
    ROOT / "bin" / "run-suite",
    ROOT / "bin" / "setup-qemu-local",
    ROOT / "bin" / "setup-toolchain",
    ROOT / "bin" / "validate",
]


def fail(message: str):
    print(f"check failed: {message}")
    raise SystemExit(1)


def load_json(path: Path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        fail(f"invalid JSON in {path.relative_to(ROOT)}: {exc}")


def main():
    missing = [str(path.relative_to(ROOT)) for path in REQUIRED_FILES if not path.exists()]
    if missing:
        fail("missing required files: " + ", ".join(missing))

    fixture_cfg = load_json(ROOT / "harness" / "config.fixture.json")
    example_cfg = load_json(ROOT / "harness" / "config.example.json")
    task = load_json(ROOT / "harness" / "cases" / "m0-fetch-summarize-post" / "task.json")

    for cfg_name, cfg in (("fixture", fixture_cfg), ("example", example_cfg)):
        if "agent_command" not in cfg or not isinstance(cfg["agent_command"], list):
            fail(f"{cfg_name} config is missing agent_command list")
        if "result_sink" not in cfg or "source_fixture" not in cfg:
            fail(f"{cfg_name} config is missing sink or source fixture settings")
        if "model_fixture" in cfg and not isinstance(cfg["model_fixture"], dict):
            fail(f"{cfg_name} config model_fixture must be an object when present")
        if "interpretation_fixture" in cfg and not isinstance(cfg["interpretation_fixture"], dict):
            fail(f"{cfg_name} config interpretation_fixture must be an object when present")
        if "translation_fixture" in cfg and not isinstance(cfg["translation_fixture"], dict):
            fail(f"{cfg_name} config translation_fixture must be an object when present")
        if "x_fixture" in cfg and not isinstance(cfg["x_fixture"], dict):
            fail(f"{cfg_name} config x_fixture must be an object when present")
        if "path_prefixes" in cfg and not isinstance(cfg["path_prefixes"], list):
            fail(f"{cfg_name} config path_prefixes must be a list when present")

    required_task_keys = {"goal_id", "goal", "expect"}
    missing_task_keys = sorted(required_task_keys.difference(task))
    if missing_task_keys:
        fail("task.json is missing keys: " + ", ".join(missing_task_keys))

    if "turns" not in task:
        for key in ("input_mode", "input_payload"):
            if key not in task:
                fail(f"task.json is missing key: {key}")

    if "{{SOURCE_URL}}" not in json.dumps(task["input_payload"]):
        fail("task.json input_payload is missing {{SOURCE_URL}} placeholder")
    if "{{RESULT_SINK_URL}}" not in json.dumps(task["input_payload"]):
        fail("task.json input_payload is missing {{RESULT_SINK_URL}} placeholder")

    if "required_trace_events" not in task["expect"]:
        fail("task.json expect is missing required_trace_events")

    print("check passed: MiniAgentOS harness structure looks good")


if __name__ == "__main__":
    main()
