from __future__ import annotations

import json


TRACE_PREFIX = "TRACE "


def substitute_placeholders(value, replacements):
    if isinstance(value, str):
        result = value
        for key, replacement in replacements.items():
            result = result.replace("{{" + key + "}}", replacement)
        return result
    if isinstance(value, list):
        return [substitute_placeholders(item, replacements) for item in value]
    if isinstance(value, dict):
        return {
            key: substitute_placeholders(item, replacements)
            for key, item in value.items()
        }
    return value


def build_input_line(input_spec, replacements):
    payload = substitute_placeholders(input_spec["input_payload"], replacements)
    mode = input_spec["input_mode"]
    if mode == "json_line":
        return json.dumps(payload, ensure_ascii=True, separators=(",", ":"))
    if mode == "text_line":
        if isinstance(payload, str):
            return payload
        return json.dumps(payload, ensure_ascii=True)
    raise ValueError(f"unsupported input_mode: {mode}")


def build_turns(case_data, replacements):
    turns = case_data.get("turns")
    if turns:
        built = []
        for turn in turns:
            built.append(
                {
                    "input_mode": turn["input_mode"],
                    "input_payload": substitute_placeholders(
                        turn["input_payload"], replacements
                    ),
                    "expect": substitute_placeholders(
                        turn.get("expect", {}), replacements
                    ),
                }
            )
        return built

    return [
        {
            "input_mode": case_data["input_mode"],
            "input_payload": substitute_placeholders(
                case_data["input_payload"], replacements
            ),
            "expect": substitute_placeholders(case_data.get("expect", {}), replacements),
        }
    ]
