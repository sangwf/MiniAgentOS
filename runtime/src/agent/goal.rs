use super::*;

const GOAL_SHELL_ID: &[u8] = b"goal-shell";
const GOAL_DEFAULT_MAX_STEPS: usize = 6;
const GOAL_DEFAULT_MODEL_URL: &[u8] = b"http://10.0.2.2:8083/summarize";
const GOAL_DEFAULT_INTERPRET_URL: &[u8] = b"http://10.0.2.2:8084/interpret";

fn ascii_eq_ignore_case(a: u8, b: u8) -> bool {
    ascii_lower(a) == ascii_lower(b)
}

fn contains_ascii_phrase(buf: &[u8], len: usize, pat: &[u8]) -> bool {
    if pat.is_empty() || len < pat.len() {
        return false;
    }
    let mut i = 0usize;
    while i + pat.len() <= len {
        let mut ok = true;
        let mut j = 0usize;
        while j < pat.len() {
            if !ascii_eq_ignore_case(buf[i + j], pat[j]) {
                ok = false;
                break;
            }
            j += 1;
        }
        if ok {
            return true;
        }
        i += 1;
    }
    false
}

fn extract_urls(buf: &[u8], len: usize, starts: &mut [usize], lens: &mut [usize]) -> usize {
    let mut found = 0usize;
    let mut i = 0usize;
    while i < len && found < starts.len() && found < lens.len() {
        let is_http = starts_with_at(buf, len, i, b"http://");
        let is_https = starts_with_at(buf, len, i, b"https://");
        if !is_http && !is_https {
            i += 1;
            continue;
        }
        let start = i;
        let mut end = i;
        while end < len && !is_space(buf[end]) {
            end += 1;
        }
        while end > start {
            let tail = buf[end - 1];
            if tail == b'.' || tail == b',' || tail == b';' || tail == b')' {
                end -= 1;
            } else {
                break;
            }
        }
        if end > start && parse_agent_url(&buf[start..end], end - start).is_some() {
            starts[found] = start;
            lens[found] = end - start;
            found += 1;
            i = end;
            continue;
        }
        i += 1;
    }
    found
}

fn detect_goal_max_items(buf: &[u8], len: usize) -> usize {
    let mut out = 3usize;
    let mut i = 0usize;
    while i < len {
        if buf[i].is_ascii_digit() {
            let mut value = 0usize;
            let start = i;
            while i < len && buf[i].is_ascii_digit() {
                value = value
                    .saturating_mul(10)
                    .saturating_add((buf[i] - b'0') as usize);
                i += 1;
            }
            if value != 0 {
                out = value;
            }
            if i > start {
                continue;
            }
        }
        i += 1;
    }
    if contains_ascii_phrase(buf, len, b"one bullet")
        || contains_ascii_phrase(buf, len, b"one point")
        || contains_ascii_phrase(buf, len, b"one item")
    {
        out = 1;
    } else if contains_ascii_phrase(buf, len, b"two bullet")
        || contains_ascii_phrase(buf, len, b"two point")
        || contains_ascii_phrase(buf, len, b"two item")
    {
        out = 2;
    } else if contains_ascii_phrase(buf, len, b"three bullet")
        || contains_ascii_phrase(buf, len, b"three point")
        || contains_ascii_phrase(buf, len, b"three item")
    {
        out = 3;
    } else if contains_ascii_phrase(buf, len, b"four bullet")
        || contains_ascii_phrase(buf, len, b"four point")
        || contains_ascii_phrase(buf, len, b"four item")
    {
        out = 4;
    } else if contains_ascii_phrase(buf, len, b"five bullet")
        || contains_ascii_phrase(buf, len, b"five point")
        || contains_ascii_phrase(buf, len, b"five item")
    {
        out = 5;
    } else if contains_ascii_phrase(buf, len, b"six bullet")
        || contains_ascii_phrase(buf, len, b"six point")
        || contains_ascii_phrase(buf, len, b"six item")
    {
        out = 6;
    }
    if out == 0 {
        3
    } else if out > 8 {
        8
    } else {
        out
    }
}

fn trace_goal_compiled() {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"goal_compiled", 0);
    uart::write_str(",\"kind\":\"agent_task\"}\n");
}

fn trace_goal_compilation_failed(reason: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"goal_compilation_failed", 0);
    uart::write_str(",\"reason\":\"");
    uart::write_bytes(reason);
    uart::write_str("\"}\n");
}

fn trace_goal_compilation_fallback(reason: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"goal_compilation_fallback", 0);
    trace_json_string_field(b"reason", reason);
    uart::write_str("}\n");
}

fn normalize_output_language(dst: &mut [u8], src: &[u8], src_len: usize) -> usize {
    if src_len == 0 {
        return copy_bytes(dst, b"default");
    }
    if contains_ascii_phrase(src, src_len, b"zh")
        || contains_ascii_phrase(src, src_len, b"chinese")
    {
        return copy_bytes(dst, b"zh");
    }
    if contains_ascii_phrase(src, src_len, b"default")
        || contains_ascii_phrase(src, src_len, b"same")
        || contains_ascii_phrase(src, src_len, b"auto")
    {
        return copy_bytes(dst, b"default");
    }
    copy_bytes(dst, &src[..src_len])
}

fn normalize_output_style(dst: &mut [u8], src: &[u8], src_len: usize) -> usize {
    if src_len == 0 {
        return copy_bytes(dst, b"bullet");
    }
    if contains_ascii_phrase(src, src_len, b"bullet")
        || contains_ascii_phrase(src, src_len, b"point")
        || contains_ascii_phrase(src, src_len, b"list")
    {
        return copy_bytes(dst, b"bullet");
    }
    if contains_ascii_phrase(src, src_len, b"paragraph") {
        return copy_bytes(dst, b"paragraph");
    }
    copy_bytes(dst, &src[..src_len])
}

fn trace_agent_loop_started() {
    trace_event(b"agent_loop_started", 0);
}

fn trace_model_interpretation_started() {
    trace_event(b"model_interpretation_started", 0);
}

fn trace_model_interpretation_result(action: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"model_interpretation_result", 0);
    trace_json_string_field(b"action", action);
    uart::write_str("}\n");
}

fn trace_intent_compiled(action: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"intent_compiled", 0);
    trace_json_string_field(b"interaction_kind", b"task");
    trace_json_string_field(b"action", action);
    unsafe {
        if AGENT_SOURCE_URL_LEN > 0 {
            trace_json_string_field(b"source_url", &AGENT_SOURCE_URL[..AGENT_SOURCE_URL_LEN]);
        }
        if AGENT_SINK_URL_LEN > 0 {
            trace_json_string_field(b"sink_url", &AGENT_SINK_URL[..AGENT_SINK_URL_LEN]);
        }
        trace_json_u64_field(b"max_items", AGENT_SUMMARY_SENTENCES as u64);
        if AGENT_OUTPUT_LANGUAGE_LEN > 0 {
            trace_json_string_field(
                b"output_language",
                &AGENT_OUTPUT_LANGUAGE[..AGENT_OUTPUT_LANGUAGE_LEN],
            );
        }
        if AGENT_OUTPUT_STYLE_LEN > 0 {
            trace_json_string_field(b"style", &AGENT_OUTPUT_STYLE[..AGENT_OUTPUT_STYLE_LEN]);
        }
    }
    trace_json_bool_field(b"requires_clarification", false);
    uart::write_str("}\n");
}

fn set_goal_shell_defaults() {
    unsafe {
        AGENT_MAX_STEPS = GOAL_DEFAULT_MAX_STEPS;
    }
}

fn copy_sink_hint_from_goal() {
    let goal = unsafe { &AGENT_GOAL_TEXT[..AGENT_GOAL_TEXT_LEN] };
    if !contains_ascii_phrase(goal, goal.len(), b"post") {
        return;
    }
    let mut starts = [0usize; 4];
    let mut lens = [0usize; 4];
    let count = extract_urls(goal, goal.len(), &mut starts, &mut lens);
    if count < 2 {
        return;
    }
    unsafe {
        AGENT_SINK_URL_LEN = copy_bytes(
            &mut AGENT_SINK_URL,
            &goal[starts[count - 1]..starts[count - 1] + lens[count - 1]],
        );
    }
}

fn compile_goal_locally() -> Result<(), &'static [u8]> {
    let goal = unsafe { &AGENT_GOAL_TEXT[..AGENT_GOAL_TEXT_LEN] };
    if contains_ascii_phrase(goal, goal.len(), b"compilation error")
        || contains_ascii_phrase(goal, goal.len(), b"ambiguous goal")
    {
        return Err(b"goal compilation failed");
    }
    let has_fetch = contains_ascii_phrase(goal, goal.len(), b"fetch");
    let has_post = contains_ascii_phrase(goal, goal.len(), b"post");
    let has_summary = contains_ascii_phrase(goal, goal.len(), b"summarize")
        || contains_ascii_phrase(goal, goal.len(), b"summary");
    if !has_summary || (!has_fetch && !has_post) {
        return Err(b"unsupported goal");
    }
    let mut starts = [0usize; 4];
    let mut lens = [0usize; 4];
    let count = extract_urls(goal, goal.len(), &mut starts, &mut lens);
    if count == 0 {
        return Err(b"unsupported goal");
    }
    unsafe {
        AGENT_SOURCE_URL_LEN =
            copy_bytes(&mut AGENT_SOURCE_URL, &goal[starts[0]..starts[0] + lens[0]]);
        AGENT_MODEL_URL_LEN = 0;
        AGENT_SINK_URL_LEN = 0;
        if has_post {
            if count < 2 {
                return Err(b"unsupported goal");
            }
            if count >= 3 {
                AGENT_MODEL_URL_LEN =
                    copy_bytes(&mut AGENT_MODEL_URL, &goal[starts[1]..starts[1] + lens[1]]);
                AGENT_SINK_URL_LEN =
                    copy_bytes(&mut AGENT_SINK_URL, &goal[starts[2]..starts[2] + lens[2]]);
            } else {
                AGENT_MODEL_URL_LEN = copy_bytes(&mut AGENT_MODEL_URL, GOAL_DEFAULT_MODEL_URL);
                AGENT_SINK_URL_LEN =
                    copy_bytes(&mut AGENT_SINK_URL, &goal[starts[1]..starts[1] + lens[1]]);
            }
        }
        AGENT_SUMMARY_SENTENCES = detect_goal_max_items(goal, goal.len());
    }
    set_goal_shell_defaults();
    if has_post {
        if !policy::agent_store_default_goal_policy() {
            return Err(b"goal policy unavailable");
        }
    } else if !policy::agent_store_local_summary_policy() {
        return Err(b"goal policy unavailable");
    }
    Ok(())
}

fn finish_goal_compilation_failure(reason: &'static [u8]) {
    let terminal = if contains_ascii_phrase(reason, reason.len(), b"unsupported") {
        AGENT_TERMINAL_REFUSED
    } else {
        AGENT_TERMINAL_FAILED
    };
    trace_goal_compilation_failed(reason);
    policy::agent_set_result(
        if terminal == AGENT_TERMINAL_REFUSED {
            b"refused"
        } else {
            b"error"
        },
        reason,
    );
    if unsafe { AGENT_SINK_URL_LEN } > 0 {
        skill::agent_start_terminal_post(terminal);
        return;
    }
    skill::agent_finish_local(0, terminal);
}

fn starts_with_fetch_goal(line: &[u8], len: usize) -> bool {
    let mut i = 0usize;
    while i < len && is_space(line[i]) {
        i += 1;
    }
    let needle = b"fetch ";
    if len < i + needle.len() {
        return false;
    }
    let mut j = 0usize;
    while j < needle.len() {
        if !ascii_eq_ignore_case(line[i + j], needle[j]) {
            return false;
        }
        j += 1;
    }
    true
}

fn finish_goal_interpretation_failure(reason: &[u8]) {
    let terminal = if contains_ascii_phrase(reason, reason.len(), b"unsupported")
        || contains_ascii_phrase(reason, reason.len(), b"missing openai key")
        || contains_ascii_phrase(reason, reason.len(), b"run openai-key")
    {
        AGENT_TERMINAL_REFUSED
    } else {
        AGENT_TERMINAL_FAILED
    };
    policy::agent_set_result(
        if terminal == AGENT_TERMINAL_REFUSED {
            b"refused"
        } else {
            b"error"
        },
        reason,
    );
    skill::agent_finish_local(0, terminal);
}

fn apply_interpretation_response() -> Result<&'static [u8], &'static [u8]> {
    let response_raw = unsafe { &AGENT_RESPONSE_BODY[..AGENT_RESPONSE_BODY_LEN] };
    let mut openai_text = [0u8; 1024];
    let response = match model::extract_openai_output_text(&mut openai_text) {
        Some(len) if len != 0 => &openai_text[..len],
        _ => response_raw,
    };
    let mut status = [0u8; 16];
    let status_len = json_extract_string(response, response.len(), b"status", &mut status)
        .ok_or(b"invalid interpretation response" as &'static [u8])?;
    if starts_with(&status[..], status_len, b"error") {
        let mut reason = [0u8; 128];
        let reason_len =
            json_extract_string(response, response.len(), b"reason", &mut reason).unwrap_or(0);
        let reason: &'static [u8] = if reason_len == 0 {
            b"interpretation failed"
        } else if starts_with(&reason[..], reason_len, b"unsupported goal") {
            b"unsupported goal"
        } else if starts_with(&reason[..], reason_len, b"translation backend unavailable")
            || starts_with(&reason[..], reason_len, b"interpretation backend unavailable")
        {
            b"interpretation backend unavailable"
        } else {
            b"interpretation failed"
        };
        return Err(reason);
    }

    let mut action = [0u8; 32];
    let action_len = json_extract_string(response, response.len(), b"action", &mut action)
        .ok_or(b"missing interpretation action" as &'static [u8])?;
    let mut source = [0u8; 256];
    let source_len = json_extract_string(response, response.len(), b"source_url", &mut source)
        .ok_or(b"missing interpreted source_url" as &'static [u8])?;
    let sink_len = unsafe {
        AGENT_SINK_URL_LEN = 0;
        json_extract_string(
            response,
            response.len(),
            b"sink_url",
            &mut AGENT_SINK_URL,
        )
        .unwrap_or(0)
    };
    let max_items = json_extract_u64(response, response.len(), b"max_items").unwrap_or(3);

    unsafe {
        AGENT_SOURCE_URL_LEN = copy_bytes(&mut AGENT_SOURCE_URL, &source[..source_len]);
        AGENT_SUMMARY_SENTENCES = if max_items == 0 {
            1
        } else if max_items > 8 {
            8
        } else {
            max_items as usize
        };
        AGENT_MODEL_URL_LEN = 0;
        AGENT_SINK_URL_LEN = sink_len;
        AGENT_MAX_STEPS = GOAL_DEFAULT_MAX_STEPS;
        AGENT_OUTPUT_STYLE_LEN = copy_bytes(&mut AGENT_OUTPUT_STYLE, b"bullet");
        AGENT_OUTPUT_LANGUAGE_LEN = copy_bytes(&mut AGENT_OUTPUT_LANGUAGE, b"default");
    }

    let mut output_language = [0u8; 16];
    let output_language_len = json_extract_string(
        response,
        response.len(),
        b"output_language",
        &mut output_language,
    )
    .unwrap_or(0);
    if output_language_len != 0 {
        unsafe {
            AGENT_OUTPUT_LANGUAGE_LEN = normalize_output_language(
                &mut AGENT_OUTPUT_LANGUAGE,
                &output_language[..output_language_len],
                output_language_len,
            );
        }
    }

    let mut style = [0u8; 16];
    let style_len = json_extract_string(response, response.len(), b"style", &mut style).unwrap_or(0);
    if style_len != 0 {
        unsafe {
            AGENT_OUTPUT_STYLE_LEN =
                normalize_output_style(&mut AGENT_OUTPUT_STYLE, &style[..style_len], style_len);
        }
    }

    if starts_with(&action[..], action_len, b"post_summary") {
        if sink_len == 0 {
            return Err(b"missing interpreted sink_url");
        }
        if !policy::agent_store_m3_post_policy() {
            return Err(b"goal policy unavailable");
        }
        Ok(b"post_summary")
    } else if starts_with(&action[..], action_len, b"local_summary") {
        if !policy::agent_store_m3_summary_policy() {
            return Err(b"goal policy unavailable");
        }
        Ok(b"local_summary")
    } else {
        Err(b"unsupported interpreted action")
    }
}

fn start_goal_interpretation() {
    unsafe {
        AGENT_MODE = AGENT_MODE_M3;
        AGENT_PHASE = AGENT_PHASE_INTERPRET_GOAL;
        AGENT_SOURCE_URL_LEN = 0;
        AGENT_SINK_URL_LEN = 0;
        AGENT_MODEL_URL_LEN = 0;
        AGENT_TASK_JSON_LEN = 0;
        AGENT_MAX_STEPS = GOAL_DEFAULT_MAX_STEPS;
        AGENT_RESPONSE_BODY_LEN = 0;
        AGENT_OUTPUT_TEXT_LEN = 0;
        AGENT_SUMMARY_LEN = 0;
        AGENT_OUTPUT_LANGUAGE_LEN = 0;
        AGENT_OUTPUT_STYLE_LEN = 0;
        AGENT_RESULT_REASON_LEN = 0;
        AGENT_OPENAI_INTERPRET_RETRIES = 0;
        AGENT_OPENAI_SUMMARY_RETRIES = 0;
        FETCH_METHOD_POST = true;
        FETCH_EXTRA_HEADER_LEN = 0;
        FETCH_OAUTH_ACTIVE = false;
    }
    trace_event(b"goal_received", 0);
    trace_event(b"goal_text_received", 0);
    trace_agent_loop_started();
    trace_model_interpretation_started();
    human_status(b"understanding goal...");
    let _ = start_openai_interpretation_request();
}

fn start_openai_interpretation_request() -> bool {
    if !crate::openai::api_key_ready() {
        finish_goal_interpretation_failure(
            b"missing openai key; run openai-key <key> before Goal > input",
        );
        return false;
    }
    let body_len = unsafe { model::build_openai_interpretation_request_body(&mut FETCH_BODY) };
    let auth_len = unsafe { crate::openai::build_bearer_header(&mut FETCH_EXTRA_HEADER) };
    if body_len == 0 {
        finish_goal_interpretation_failure(b"openai interpretation request too large");
        return false;
    }
    if auth_len == 0 {
        finish_goal_interpretation_failure(
            b"missing openai key; run openai-key <key> before Goal > input",
        );
        return false;
    }
    unsafe {
        AGENT_RESPONSE_BODY_LEN = 0;
        AGENT_OUTPUT_TEXT_LEN = 0;
        FETCH_METHOD_POST = true;
        FETCH_BODY_LEN = body_len;
        FETCH_EXTRA_HEADER_LEN = auth_len;
        FETCH_OAUTH_ACTIVE = true;
    }
    let started =
        skill::fetch_start_agent_url(crate::openai::responses_url(), [10, 0, 2, 15], [0, 0, 0, 0], 0);
    if !started {
        finish_goal_interpretation_failure(fetch_failure_reason_or(
            b"openai interpretation request failed",
        ));
    }
    started
}

pub(crate) fn handle_goal_line(line: &[u8], len: usize) -> bool {
    if len == 0 {
        return false;
    }
    if unsafe { AGENT_TASK_ACTIVE || FETCH_STATE != FETCH_IDLE } {
        uart::write_str("busy\n");
        return true;
    }
    unsafe {
        AGENT_TASK_ACTIVE = true;
        AGENT_MODE = AGENT_MODE_M2;
        AGENT_PHASE = AGENT_PHASE_IDLE;
        AGENT_GOAL_ID_LEN = copy_bytes(&mut AGENT_GOAL_ID, GOAL_SHELL_ID);
        AGENT_GOAL_TEXT_LEN = copy_bytes(&mut AGENT_GOAL_TEXT, &line[..len]);
        AGENT_SOURCE_URL_LEN = 0;
        AGENT_SINK_URL_LEN = 0;
        AGENT_MODEL_URL_LEN = 0;
        AGENT_TASK_JSON_LEN = 0;
        AGENT_MAX_STEPS = 0;
        AGENT_RESPONSE_BODY_LEN = 0;
        AGENT_OUTPUT_TEXT_LEN = 0;
        AGENT_SUMMARY_LEN = 0;
        AGENT_OUTPUT_LANGUAGE_LEN = 0;
        AGENT_OUTPUT_STYLE_LEN = 0;
        AGENT_RESULT_REASON_LEN = 0;
        AGENT_OPENAI_INTERPRET_RETRIES = 0;
        AGENT_OPENAI_SUMMARY_RETRIES = 0;
        FETCH_METHOD_POST = false;
        FETCH_BODY_LEN = 0;
        FETCH_EXTRA_HEADER_LEN = 0;
        FETCH_OAUTH_ACTIVE = false;
    }
    if starts_with_fetch_goal(line, len) {
        copy_sink_hint_from_goal();
        trace_event(b"goal_received", 0);
        trace_event(b"goal_text_received", 0);
        trace_event(b"goal_compilation_started", 0);
        match compile_goal_locally() {
            Ok(()) => {
                trace_goal_compiled();
                if unsafe { AGENT_SINK_URL_LEN } > 0 {
                    task::start_governed_m1_task();
                } else {
                    task::start_local_summary_task();
                }
            }
            Err(reason) => {
                if starts_with(reason, reason.len(), b"unsupported goal")
                    || starts_with(reason, reason.len(), b"goal compilation failed")
                {
                    trace_goal_compilation_fallback(reason);
                    start_goal_interpretation();
                } else {
                    finish_goal_compilation_failure(reason);
                }
            }
        }
    } else {
        start_goal_interpretation();
    }
    true
}

pub(super) fn handle_goal_interpretation_done(ok: bool) -> bool {
    if unsafe { AGENT_MODE } != AGENT_MODE_M3 || unsafe { AGENT_PHASE } != AGENT_PHASE_INTERPRET_GOAL {
        return false;
    }
    if !model::agent_http_success(ok) {
        if model::openai_failure_retryable()
            && unsafe { AGENT_OPENAI_INTERPRET_RETRIES } < AGENT_OPENAI_MAX_RETRIES
        {
            unsafe {
                AGENT_OPENAI_INTERPRET_RETRIES = AGENT_OPENAI_INTERPRET_RETRIES.wrapping_add(1);
            }
            trace_retry_scheduled(0, b"openai_interpretation", unsafe {
                AGENT_OPENAI_INTERPRET_RETRIES
            });
            human_status(b"retrying interpretation...");
            let _ = start_openai_interpretation_request();
            return true;
        }
        if crate::openai::api_key_ready() {
            let mut reason = [0u8; 160];
            let reason_len = model::build_openai_failure_reason(&mut reason, b"openai interpretation");
            finish_goal_interpretation_failure(&reason[..reason_len]);
        } else {
            finish_goal_interpretation_failure(b"interpretation request failed");
        }
        return true;
    }
    if model::agent_response_body_truncated() {
        finish_goal_interpretation_failure(b"openai interpretation: response body truncated");
        return true;
    }
    match apply_interpretation_response() {
        Ok(action) => {
            trace_model_interpretation_result(action);
            trace_intent_compiled(action);
            task::start_m3_local_summary_task();
        }
        Err(reason) => finish_goal_interpretation_failure(reason),
    }
    true
}
