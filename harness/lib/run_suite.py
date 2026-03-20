from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path


def _load_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def _case_requires_guest_openai(case_path: Path) -> bool:
    try:
        case_data = _load_json(case_path)
    except json.JSONDecodeError:
        return False
    return bool(case_data.get("requires_guest_openai"))


def _case_skip_when_guest_openai(case_path: Path) -> bool:
    try:
        case_data = _load_json(case_path)
    except json.JSONDecodeError:
        return False
    return bool(case_data.get("skip_when_guest_openai"))


def _include_guest_openai_cases(root: Path, config_path: str) -> bool:
    try:
        config = _load_json((root / config_path).resolve())
    except (FileNotFoundError, json.JSONDecodeError):
        return False
    env_name = config.get("guest_openai_key_env")
    if not env_name:
        return False
    env_name = str(env_name)
    value = os.environ.get(env_name)
    if value:
        return True
    for shell in ("zsh", "bash"):
        try:
            proc = subprocess.run(
                [shell, "-ic", f'printf %s "${{{env_name}:-}}"'],
                capture_output=True,
                text=True,
                timeout=5,
            )
        except (FileNotFoundError, subprocess.TimeoutExpired):
            continue
        if proc.returncode == 0 and proc.stdout:
            return True
    return False


def _suite_cases(root: Path, suite: str, include_guest_openai: bool) -> list[Path]:
    cases_dir = root / "harness" / "cases"
    cases: list[Path] = []
    for path in sorted(cases_dir.glob(f"{suite}-*")):
        if not path.is_dir():
            continue
        case_path = path / "task.json"
        if not case_path.exists():
            continue
        if _case_requires_guest_openai(case_path) and not include_guest_openai:
            continue
        if _case_skip_when_guest_openai(case_path) and include_guest_openai:
            continue
        cases.append(case_path)
    return cases


def _resolve_case(root: Path, raw: str) -> Path:
    path = (root / raw).resolve()
    if path.is_dir():
        return path / "task.json"
    return path


def main():
    parser = argparse.ArgumentParser(description="Run a MiniAgentOS harness suite")
    parser.add_argument(
        "cases",
        nargs="*",
        help="Case task.json paths or case directories. Defaults to every case in the selected suite.",
    )
    parser.add_argument("--config", required=True, help="Harness config path")
    parser.add_argument(
        "--suite",
        choices=("m1", "m2", "m3", "m4", "m5", "m5live", "m6", "m6live", "m7", "m7live"),
        default="m1",
        help="Default suite prefix to use when no explicit cases are provided.",
    )
    parser.add_argument(
        "--output-root",
        help="Output directory root for suite artifacts. Defaults to output/<suite>-suite.",
    )
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[2]
    config_path = str((root / args.config).resolve())
    output_root_arg = args.output_root or f"output/{args.suite}-suite"
    output_root = (root / output_root_arg).resolve()
    output_root.mkdir(parents=True, exist_ok=True)
    include_guest_openai = _include_guest_openai_cases(root, args.config)

    cases = (
        [_resolve_case(root, raw) for raw in args.cases]
        if args.cases
        else _suite_cases(root, args.suite, include_guest_openai)
    )
    if not cases:
        print("run-suite: no cases matched", file=sys.stderr)
        return 1

    failures: list[str] = []
    for case_path in cases:
        case_name = case_path.parent.name
        output_dir = output_root / case_name
        command = [
            str(root / "bin" / "run-case"),
            str(case_path),
            "--config",
            config_path,
            "--output",
            str(output_dir),
        ]
        result = subprocess.run(command, cwd=root)
        status = "PASS" if result.returncode == 0 else "FAIL"
        print(f"{status} {case_name}")
        if result.returncode != 0:
            failures.append(case_name)

    if failures:
        print("suite failed: " + ", ".join(failures), file=sys.stderr)
        return 1
    print("suite passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
