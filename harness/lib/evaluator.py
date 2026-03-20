from __future__ import annotations

import re


def _event_names(trace_events):
    return [event.get("event") for event in trace_events if isinstance(event, dict)]


def _contains_required_sequence(actual_names, expected_names):
    if not expected_names:
        return True
    index = 0
    for name in actual_names:
        if name == expected_names[index]:
            index += 1
            if index == len(expected_names):
                return True
    return False


def _compare_expected_subset(actual, expected, prefix, failures):
    if isinstance(expected, dict):
        if not isinstance(actual, dict):
            failures.append(f"{prefix} is missing or not an object")
            return
        for key, expected_value in expected.items():
            if key not in actual:
                failures.append(f"{prefix} is missing field: {key}")
                continue
            child_prefix = f"{prefix}.{key}" if prefix else key
            _compare_expected_subset(actual.get(key), expected_value, child_prefix, failures)
        return
    if actual != expected:
        failures.append(f"unexpected {prefix}: {actual!r} != {expected!r}")


def _contains_expected_event_subset(actual_events, expected_events):
    if not expected_events:
        return True
    index = 0
    for actual in actual_events:
        local_failures = []
        _compare_expected_subset(actual, expected_events[index], "tool_call", local_failures)
        if not local_failures:
            index += 1
            if index == len(expected_events):
                return True
    return False


def _workspace_file_content(snapshot, path):
    if not isinstance(snapshot, dict):
        return None
    entries = snapshot.get("entries")
    if not isinstance(entries, dict):
        return None
    entry = entries.get(path)
    if not isinstance(entry, dict):
        return None
    return entry.get("content")


def evaluate_case(
    case_data,
    trace_events,
    result_payload,
    prompt_seen,
    observations=None,
    uart_text="",
    terminal_result=None,
    intent_ir=None,
    session_transcript=None,
    tool_calls=None,
    workspace_before=None,
    workspace_after=None,
    process_runs=None,
    tool_errors=None,
    search_results=None,
    fetched_sources=None,
    source_memory=None,
):
    expect = case_data["expect"]
    failures = []
    observations = observations or {}
    session_transcript = session_transcript or []
    tool_calls = tool_calls or []
    process_runs = process_runs or []
    tool_errors = tool_errors or []
    search_results = search_results or {}
    fetched_sources = fetched_sources or {}
    source_memory = source_memory or {}

    guest_exception = observations.get("guest_exception")
    if guest_exception:
        failures.append(f"guest exception observed: {guest_exception}")

    if not prompt_seen:
        failures.append("boot prompt was not observed")

    result_required = expect.get("result_required", True)
    if result_payload is None:
        if result_required:
            failures.append("result sink did not receive a payload")
    else:
        for field in expect.get("result_fields", []):
            if field not in result_payload:
                failures.append(f"result payload is missing field: {field}")
        expected_status = expect.get("result_status")
        if expected_status and result_payload.get("status") != expected_status:
            failures.append(
                f"unexpected result status: {result_payload.get('status')!r}"
            )
        summary_required = expect.get("summary_required")
        if summary_required is None:
            summary_required = (
                "summary" in expect.get("result_fields", [])
                or bool(expect.get("summary_must_include"))
            )
        summary = result_payload.get("summary", "")
        if summary_required and (not isinstance(summary, str) or not summary.strip()):
            failures.append("result summary is empty")
        for needle in expect.get("summary_must_include", []):
            if needle not in summary:
                failures.append(f"summary is missing required term: {needle}")
        reason_required = expect.get("reason_required")
        if reason_required is None:
            reason_required = (
                "reason" in expect.get("result_fields", [])
                or bool(expect.get("reason_must_include"))
            )
        reason = result_payload.get("reason", "")
        if reason_required and (not isinstance(reason, str) or not reason.strip()):
            failures.append("result reason is empty")
        for needle in expect.get("reason_must_include", []):
            if needle not in reason:
                failures.append(f"result reason is missing required term: {needle}")

    for needle in expect.get("uart_must_include", []):
        if needle not in uart_text:
            failures.append(f"uart output is missing required term: {needle}")

    terminal_result = terminal_result or {}
    terminal_required = expect.get("terminal_result_required", False)
    if terminal_required and not terminal_result:
        failures.append("terminal_result artifact was not captured")
    if terminal_result:
        for field in expect.get("terminal_fields", []):
            if field not in terminal_result:
                failures.append(f"terminal result is missing field: {field}")
        expected_terminal_status = expect.get("terminal_status")
        if expected_terminal_status and terminal_result.get("status") != expected_terminal_status:
            failures.append(
                f"unexpected terminal status: {terminal_result.get('status')!r}"
            )
        terminal_summary_required = expect.get("terminal_summary_required")
        if terminal_summary_required is None:
            terminal_summary_required = (
                "summary" in expect.get("terminal_fields", [])
                or bool(expect.get("terminal_summary_must_include"))
            )
        terminal_summary = terminal_result.get("summary", "")
        if terminal_summary_required and (
            not isinstance(terminal_summary, str) or not terminal_summary.strip()
        ):
            failures.append("terminal summary is empty")
        for needle in expect.get("terminal_summary_must_include", []):
            if needle not in terminal_summary:
                failures.append(
                    f"terminal summary is missing required term: {needle}"
                )
        for pattern in expect.get("terminal_summary_must_match_regex", []):
            if re.search(pattern, terminal_summary) is None:
                failures.append(
                    f"terminal summary did not match required regex: {pattern}"
                )
        terminal_reason_required = expect.get("terminal_reason_required")
        if terminal_reason_required is None:
            terminal_reason_required = (
                "reason" in expect.get("terminal_fields", [])
                or bool(expect.get("terminal_reason_must_include"))
            )
        terminal_reason = terminal_result.get("reason", "")
        if terminal_reason_required and (
            not isinstance(terminal_reason, str) or not terminal_reason.strip()
        ):
            failures.append("terminal reason is empty")
        for needle in expect.get("terminal_reason_must_include", []):
            if needle not in terminal_reason:
                failures.append(
                    f"terminal reason is missing required term: {needle}"
                )

    intent_required = expect.get("intent_required")
    expected_intent = expect.get("expected_intent")
    if intent_required is None:
        intent_required = bool(expected_intent)
    if expect.get("intent_absent"):
        if intent_ir:
            failures.append("intent_ir was captured unexpectedly")
    else:
        if intent_required and not intent_ir:
            failures.append("intent_ir artifact was not captured")
        if intent_ir:
            for field in expect.get("intent_fields", []):
                if field not in intent_ir:
                    failures.append(f"intent_ir is missing field: {field}")
            if expected_intent:
                _compare_expected_subset(intent_ir, expected_intent, "intent_ir", failures)

    session_transcript_required = expect.get("session_transcript_required", False)
    if session_transcript_required and not session_transcript:
        failures.append("session_transcript artifact was not captured")
    expected_turn_count = expect.get("expected_turn_count")
    if expected_turn_count is not None and len(session_transcript) != expected_turn_count:
        failures.append(
            f"unexpected session turn count: {len(session_transcript)} != {expected_turn_count}"
        )

    expected_tool_calls = expect.get("expected_tool_calls", [])
    if expected_tool_calls and not _contains_expected_event_subset(
        tool_calls, expected_tool_calls
    ):
        failures.append("tool calls did not include the expected ordered subset")

    trace_names = _event_names(trace_events)
    if not trace_names:
        failures.append("no trace events were captured")
    elif not _contains_required_sequence(
        trace_names, expect.get("required_trace_events", [])
    ):
        failures.append(
            "trace events did not include the required ordered sequence"
        )

    max_skill_calls = expect.get("max_skill_calls")
    if max_skill_calls is not None:
        skill_calls = sum(1 for name in trace_names if name == "skill_called")
        if skill_calls > max_skill_calls:
            failures.append(
                f"skill call budget exceeded: {skill_calls} > {max_skill_calls}"
            )

    max_tool_calls = expect.get("max_tool_calls")
    if max_tool_calls is not None:
        requested_calls = sum(
            1 for event in tool_calls if event.get("event") == "tool_call_requested"
        )
        if requested_calls > max_tool_calls:
            failures.append(
                f"tool call budget exceeded: {requested_calls} > {max_tool_calls}"
            )

    for key, expected_value in expect.get("request_counts", {}).items():
        actual_value = observations.get(key)
        if actual_value != expected_value:
            failures.append(
                f"unexpected request count for {key}: {actual_value} != {expected_value}"
            )

    for path, expected_content in expect.get("workspace_files_before", {}).items():
        actual_content = _workspace_file_content(workspace_before, path)
        if actual_content != expected_content:
            failures.append(
                f"unexpected workspace_before content for {path}: {actual_content!r} != {expected_content!r}"
            )

    for path, expected_content in expect.get("workspace_files_after", {}).items():
        actual_content = _workspace_file_content(workspace_after, path)
        if actual_content != expected_content:
            failures.append(
                f"unexpected workspace_after content for {path}: {actual_content!r} != {expected_content!r}"
            )

    if expect.get("process_runs_required") and not process_runs:
        failures.append("process_runs artifact was not captured")
    expected_process_run_count = expect.get("process_run_count")
    if expected_process_run_count is not None and len(process_runs) != expected_process_run_count:
        failures.append(
            f"unexpected process run count: {len(process_runs)} != {expected_process_run_count}"
        )
    expected_process_runs = expect.get("expected_process_runs", [])
    if expected_process_runs and not _contains_expected_event_subset(
        process_runs, expected_process_runs
    ):
        failures.append("process runs did not include the expected ordered subset")

    if expect.get("tool_errors_required") and not tool_errors:
        failures.append("tool_errors artifact was not captured")
    expected_tool_error_count = expect.get("tool_error_count")
    if expected_tool_error_count is not None and len(tool_errors) != expected_tool_error_count:
        failures.append(
            f"unexpected tool error count: {len(tool_errors)} != {expected_tool_error_count}"
        )
    expected_tool_errors = expect.get("expected_tool_errors", [])
    if expected_tool_errors and not _contains_expected_event_subset(
        tool_errors, expected_tool_errors
    ):
        failures.append("tool errors did not include the expected ordered subset")

    searches = search_results.get("searches", []) if isinstance(search_results, dict) else []
    if expect.get("search_results_required") and not searches:
        failures.append("search_results artifact was not captured")
    expected_search_count = expect.get("search_count")
    if expected_search_count is not None and len(searches) != expected_search_count:
        failures.append(
            f"unexpected search count: {len(searches)} != {expected_search_count}"
        )
    expected_searches = expect.get("expected_searches", [])
    if expected_searches and not _contains_expected_event_subset(
        searches, expected_searches
    ):
        failures.append("search results did not include the expected ordered subset")

    fetched = fetched_sources.get("sources", []) if isinstance(fetched_sources, dict) else []
    if expect.get("fetched_sources_required") and not fetched:
        failures.append("fetched_sources artifact was not captured")
    expected_fetched_source_count = expect.get("fetched_source_count")
    if expected_fetched_source_count is not None and len(fetched) != expected_fetched_source_count:
        failures.append(
            f"unexpected fetched source count: {len(fetched)} != {expected_fetched_source_count}"
        )
    expected_fetched_sources = expect.get("expected_fetched_sources", [])
    if expected_fetched_sources and not _contains_expected_event_subset(
        fetched, expected_fetched_sources
    ):
        failures.append("fetched sources did not include the expected ordered subset")

    remembered_sources = (
        source_memory.get("sources", []) if isinstance(source_memory, dict) else []
    )
    if expect.get("source_memory_required") and not remembered_sources:
        failures.append("source_memory artifact was not captured")
    expected_source_memory_count = expect.get("source_memory_count")
    if expected_source_memory_count is not None and len(remembered_sources) != expected_source_memory_count:
        failures.append(
            f"unexpected source memory count: {len(remembered_sources)} != {expected_source_memory_count}"
        )
    expected_source_memory = expect.get("expected_source_memory", [])
    if expected_source_memory and not _contains_expected_event_subset(
        remembered_sources, expected_source_memory
    ):
        failures.append("source memory did not include the expected ordered subset")

    return {
        "pass": not failures,
        "failures": failures,
        "trace_events": trace_names,
        "observations": observations,
        "terminal_result": terminal_result,
        "intent_ir": intent_ir,
        "session_transcript": session_transcript,
        "tool_calls": tool_calls,
        "workspace_before": workspace_before,
        "workspace_after": workspace_after,
        "process_runs": process_runs,
        "tool_errors": tool_errors,
        "search_results": search_results,
        "fetched_sources": fetched_sources,
        "source_memory": source_memory,
    }
