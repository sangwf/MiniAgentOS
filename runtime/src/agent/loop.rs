use super::*;

const M4_LOOP_MAX_STEPS: u8 = 4;
const M4_OPENAI_MAX_RETRIES: u8 = 4;
const M4_USER_TURN_COOLDOWN_MS: u64 = 3_000;
const M4_USER_TURN_FAILURE_COOLDOWN_MS: u64 = 10_000;
const M4_MODEL_INPUT_LIMIT: usize = 8_192;
const M4_TOOL_RESULT_LIMIT: usize = 8_192;
const M4_PROMPT_BUF_LIMIT: usize = 16_384;
const M4_CURRENT_REQUEST_LIMIT: usize = 1_024;
const M4_LATEST_TOOL_RESULT_LIMIT: usize = 2_048;
const M4_STATE_SNAPSHOT_LIMIT: usize = 1_536;
const M4_RECENT_CONVERSATION_LIMIT: usize = 3_072;
const M4_FETCH_PREVIEW_LIMIT: usize = 900;

static mut M4_TOOL_NAME: [u8; 32] = [0u8; 32];
static mut M4_TOOL_NAME_LEN: usize = 0;
static mut M4_TOOL_ARG1: [u8; 512] = [0u8; 512];
static mut M4_TOOL_ARG1_LEN: usize = 0;
static mut M4_TOOL_ARG2: [u8; 768] = [0u8; 768];
static mut M4_TOOL_ARG2_LEN: usize = 0;
static mut M4_LOOP_STEP: u8 = 0;
static mut M4_MODEL_RETRIES: u8 = 0;
static mut M4_OPENAI_COOLDOWN_UNTIL_MS: u64 = 0;
static mut M4_USER_TURN_COOLDOWN_UNTIL_MS: u64 = 0;
static mut M4_LAST_TOOL_RESULT: [u8; M4_TOOL_RESULT_LIMIT] = [0u8; M4_TOOL_RESULT_LIMIT];
static mut M4_LAST_TOOL_RESULT_LEN: usize = 0;
static mut M4_PATH_BUF: [u8; 512] = [0u8; 512];
static mut M4_LAST_FETCH_URL: [u8; 512] = [0u8; 512];
static mut M4_LAST_FETCH_URL_LEN: usize = 0;
static mut M4_LAST_FETCH_BODY: [u8; M4_TOOL_RESULT_LIMIT] = [0u8; M4_TOOL_RESULT_LIMIT];
static mut M4_LAST_FETCH_BODY_LEN: usize = 0;
static mut M4_PROMPT_BUF: [u8; M4_PROMPT_BUF_LIMIT] = [0u8; M4_PROMPT_BUF_LIMIT];
static mut M4_MODEL_TEXT_BUF: [u8; 16384] = [0u8; 16384];

fn copy_trimmed_text(out: &mut [u8], src: &[u8]) -> usize {
    let mut start = 0usize;
    let mut end = src.len();
    while start < end && is_space(src[start]) {
        start += 1;
    }
    while end > start && is_space(src[end - 1]) {
        end -= 1;
    }
    copy_bytes(out, &src[start..end])
}

fn json_extract_string_local(buf: &[u8], key: &[u8], out: &mut [u8]) -> usize {
    json_extract_string(buf, buf.len(), key, out).unwrap_or(0)
}

fn json_extract_string_partial_local(buf: &[u8], key: &[u8], out: &mut [u8]) -> usize {
    let mut i = 0usize;
    while i + key.len() + 2 < buf.len() {
        if buf[i] != b'"' {
            i += 1;
            continue;
        }
        let mut matched = true;
        let mut j = 0usize;
        while j < key.len() {
            if i + 1 + j >= buf.len() || buf[i + 1 + j] != key[j] {
                matched = false;
                break;
            }
            j += 1;
        }
        if !matched || i + 1 + key.len() >= buf.len() || buf[i + 1 + key.len()] != b'"' {
            i += 1;
            continue;
        }
        let mut off = i + key.len() + 2;
        while off < buf.len() && is_space(buf[off]) {
            off += 1;
        }
        if off >= buf.len() || buf[off] != b':' {
            i += 1;
            continue;
        }
        off += 1;
        while off < buf.len() && is_space(buf[off]) {
            off += 1;
        }
        if off >= buf.len() || buf[off] != b'"' {
            i += 1;
            continue;
        }
        off += 1;
        let mut out_len = 0usize;
        while off < buf.len() && out_len < out.len() {
            let b = buf[off];
            off += 1;
            if b == b'"' {
                return out_len;
            }
            if b != b'\\' {
                out[out_len] = b;
                out_len += 1;
                continue;
            }
            if off >= buf.len() {
                return out_len;
            }
            let esc = buf[off];
            off += 1;
            match esc {
                b'"' | b'\\' | b'/' => {
                    out[out_len] = esc;
                    out_len += 1;
                }
                b'b' => {
                    out[out_len] = 0x08;
                    out_len += 1;
                }
                b'f' => {
                    out[out_len] = 0x0c;
                    out_len += 1;
                }
                b'n' => {
                    out[out_len] = b'\n';
                    out_len += 1;
                }
                b'r' => {
                    out[out_len] = b'\r';
                    out_len += 1;
                }
                b't' => {
                    out[out_len] = b'\t';
                    out_len += 1;
                }
                b'u' => {
                    if off + 4 <= buf.len() {
                        let d0 = match hex_nibble(buf[off]) {
                            Some(v) => v as u32,
                            None => return out_len,
                        };
                        let d1 = match hex_nibble(buf[off + 1]) {
                            Some(v) => v as u32,
                            None => return out_len,
                        };
                        let d2 = match hex_nibble(buf[off + 2]) {
                            Some(v) => v as u32,
                            None => return out_len,
                        };
                        let d3 = match hex_nibble(buf[off + 3]) {
                            Some(v) => v as u32,
                            None => return out_len,
                        };
                        off += 4;
                        let cp = (d0 << 12) | (d1 << 8) | (d2 << 4) | d3;
                        append_utf8_codepoint(out, &mut out_len, cp);
                    } else {
                        return out_len;
                    }
                }
                _ => {
                    out[out_len] = esc;
                    out_len += 1;
                }
            }
        }
        return out_len;
    }
    0
}

fn utf8_safe_suffix_start(buf: &[u8], keep: usize) -> usize {
    if buf.len() <= keep {
        return 0;
    }
    let mut start = buf.len() - keep;
    while start < buf.len() && is_utf8_continuation_byte(buf[start]) {
        start += 1;
    }
    if start >= buf.len() {
        buf.len()
    } else {
        start
    }
}

fn m4_reset_turn_state() {
    unsafe {
        M4_TOOL_NAME_LEN = 0;
        M4_TOOL_ARG1_LEN = 0;
        M4_TOOL_ARG2_LEN = 0;
        M4_LAST_TOOL_RESULT_LEN = 0;
        M4_MODEL_RETRIES = 0;
        M4_LOOP_STEP = 0;
        M4_LAST_FETCH_URL_LEN = 0;
        M4_LAST_FETCH_BODY_LEN = 0;
    }
}

fn m4_now_ms() -> u64 {
    crate::timer::ticks_to_ms(crate::timer::counter_ticks(), crate::timer::counter_freq_hz())
}

fn m4_retry_backoff_ms(attempt: u8) -> u64 {
    match attempt {
        0 | 1 => 700,
        2 => 1_600,
        3 => 3_200,
        _ => 5_000,
    }
}

fn m4_apply_openai_cooldown(message: &[u8]) {
    let now = m4_now_ms();
    let wait_ms = unsafe { M4_OPENAI_COOLDOWN_UNTIL_MS.saturating_sub(now) };
    if wait_ms != 0 {
        human_status(message);
        crate::timer::delay_ms(wait_ms);
    }
    unsafe {
        M4_OPENAI_COOLDOWN_UNTIL_MS = 0;
    }
}

fn m4_prepare_openai_attempt(message: &[u8]) {
    let retry_backoff = unsafe {
        if M4_MODEL_RETRIES == 0 {
            0
        } else {
            m4_retry_backoff_ms(M4_MODEL_RETRIES)
        }
    };
    m4_apply_openai_cooldown(message);
    if retry_backoff != 0 {
        human_status(message);
        crate::timer::delay_ms(retry_backoff);
    }
    crate::fetch_prepare_openai_transport();
}

fn m4_schedule_user_turn_cooldown() {
    unsafe {
        M4_USER_TURN_COOLDOWN_UNTIL_MS = m4_now_ms().saturating_add(M4_USER_TURN_COOLDOWN_MS);
    }
}

fn m4_schedule_user_turn_failure_cooldown() {
    let until = m4_now_ms().saturating_add(M4_USER_TURN_FAILURE_COOLDOWN_MS);
    unsafe {
        if until > M4_USER_TURN_COOLDOWN_UNTIL_MS {
            M4_USER_TURN_COOLDOWN_UNTIL_MS = until;
        }
    }
}

fn m4_apply_user_turn_cooldown() {
    let now = m4_now_ms();
    let wait_ms = unsafe { M4_USER_TURN_COOLDOWN_UNTIL_MS.saturating_sub(now) };
    if wait_ms != 0 {
        crate::timer::delay_ms(wait_ms);
    }
    unsafe {
        M4_USER_TURN_COOLDOWN_UNTIL_MS = 0;
    }
}

fn m4_mark_retryable_openai_failure() {
    let cooldown_ms = m4_retry_backoff_ms(M4_OPENAI_MAX_RETRIES.wrapping_add(1));
    unsafe {
        M4_OPENAI_COOLDOWN_UNTIL_MS = m4_now_ms().saturating_add(cooldown_ms);
    }
    m4_schedule_user_turn_failure_cooldown();
    crate::tls::hard_reset();
}

pub(super) fn reset_m4_state() {
    m4_reset_turn_state();
}

pub(super) fn current_loop_step() -> u8 {
    unsafe { M4_LOOP_STEP }
}

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

fn contains_bytes_phrase(buf: &[u8], len: usize, pat: &[u8]) -> bool {
    if pat.is_empty() || len < pat.len() {
        return false;
    }
    let mut i = 0usize;
    while i + pat.len() <= len {
        let mut ok = true;
        let mut j = 0usize;
        while j < pat.len() {
            if buf[i + j] != pat[j] {
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

fn contains_non_ascii(buf: &[u8]) -> bool {
    let mut i = 0usize;
    while i < buf.len() {
        if buf[i] & 0x80 != 0 {
            return true;
        }
        i += 1;
    }
    false
}

fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0usize;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

fn is_m4_candidate(line: &[u8], len: usize) -> bool {
    len != 0 && !starts_with(&line[..], len, b"goal ") && !starts_with(&line[..], len, b"m3 ")
}

fn starts_with_ignore_leading_space(line: &[u8], len: usize, pat: &[u8]) -> bool {
    let mut i = 0usize;
    while i < len && is_space(line[i]) {
        i += 1;
    }
    starts_with_at(line, len, i, pat)
}

fn looks_like_tool_inventory_request(goal: &[u8]) -> bool {
    contains_ascii_phrase(goal, goal.len(), b"what tools")
        || contains_ascii_phrase(goal, goal.len(), b"which tools")
        || contains_ascii_phrase(goal, goal.len(), b"available tools")
        || contains_ascii_phrase(goal, goal.len(), b"tool list")
        || contains_ascii_phrase(goal, goal.len(), b"what can you do")
        || contains_ascii_phrase(goal, goal.len(), b"your capabilities")
        || contains_ascii_phrase(goal, goal.len(), b"what are your capabilities")
        || contains_bytes_phrase(goal, goal.len(), "什么工具".as_bytes())
        || contains_bytes_phrase(goal, goal.len(), "哪些工具".as_bytes())
        || contains_bytes_phrase(goal, goal.len(), "有什么工具".as_bytes())
        || contains_bytes_phrase(goal, goal.len(), "你都有什么工具".as_bytes())
        || contains_bytes_phrase(goal, goal.len(), "工具可被调用".as_bytes())
        || contains_bytes_phrase(goal, goal.len(), "可以调用".as_bytes())
        || contains_bytes_phrase(goal, goal.len(), "能调用哪些工具".as_bytes())
        || contains_bytes_phrase(goal, goal.len(), "都有什么能力".as_bytes())
}

fn build_tool_inventory_response(goal: &[u8], out: &mut [u8]) -> usize {
    if contains_non_ascii(goal) {
        copy_bytes(
            out,
            "我当前可调用的工具有：fetch_url（抓取一个 URL）、post_url（向允许的端点发送 JSON POST）、post_tweet（发布一条推文）、search_recent_posts（搜索近期推文）、get_user_posts（读取某个账号的近期推文）、read_session_state（读取会话状态）、write_session_state（写入会话状态）。".as_bytes(),
        )
    } else {
        copy_bytes(
            out,
            b"My callable tools are: fetch_url (fetch one URL), post_url (send one allowed JSON POST), post_tweet (publish one tweet), search_recent_posts (search recent posts), get_user_posts (read one account's recent posts), read_session_state (read session state), and write_session_state (write session state).",
        )
    }
}

fn try_handle_local_meta_request(goal: &[u8]) -> bool {
    if !looks_like_tool_inventory_request(goal) {
        return false;
    }
    let mut response = [0u8; 768];
    let response_len = build_tool_inventory_response(goal, &mut response);
    if response_len == 0 {
        return false;
    }
    finalize_m4_response(&response[..response_len], false);
    true
}

pub(crate) fn handle_session_command(line: &[u8], len: usize) -> bool {
    if starts_with_ignore_leading_space(line, len, b"session new")
        || starts_with_ignore_leading_space(line, len, b"session reset")
        || starts_with_ignore_leading_space(line, len, b"session clear")
    {
        session::session_reset();
        clear_inline_status();
        uart::write_str("session reset\n");
        return true;
    }
    if starts_with_ignore_leading_space(line, len, b"session status") {
        session::session_status();
        return true;
    }
    false
}

fn build_search_path(query: &[u8], out: &mut [u8]) -> usize {
    let mut idx = 0usize;
    idx = copy_bytes(&mut out[idx..], X_SEARCH_RECENT_PATH_PREFIX) + idx;
    append_urlencoded(out, &mut idx, query);
    idx = copy_bytes(&mut out[idx..], X_SEARCH_RECENT_PATH_SUFFIX) + idx;
    idx
}

fn build_user_posts_query(username: &[u8], out: &mut [u8]) -> usize {
    let mut idx = 0usize;
    idx = copy_bytes(&mut out[idx..], b"from:") + idx;
    idx = copy_bytes(&mut out[idx..], username) + idx;
    idx = copy_bytes(&mut out[idx..], b" -is:retweet") + idx;
    idx
}

fn trace_tool_call_with_arg(event: &[u8], tool: &[u8], arg_name: &[u8], arg_value: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(event, unsafe { M4_LOOP_STEP });
    trace_json_string_field(b"tool", tool);
    uart::write_str(",\"arguments\":{\"");
    uart::write_bytes(arg_name);
    uart::write_str("\":\"");
    trace_json_escaped(arg_value);
    uart::write_str("\"}}\n");
}

fn trace_tool_call_with_two_args(
    event: &[u8],
    tool: &[u8],
    arg1_name: &[u8],
    arg1_value: &[u8],
    arg2_name: &[u8],
    arg2_value: &[u8],
) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(event, unsafe { M4_LOOP_STEP });
    trace_json_string_field(b"tool", tool);
    uart::write_str(",\"arguments\":{\"");
    uart::write_bytes(arg1_name);
    uart::write_str("\":\"");
    trace_json_escaped(arg1_value);
    uart::write_str("\",\"");
    uart::write_bytes(arg2_name);
    uart::write_str("\":\"");
    trace_json_escaped(arg2_value);
    uart::write_str("\"}}\n");
}

fn trace_tool_call_completed(tool: &[u8], status: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"tool_call_completed", unsafe { M4_LOOP_STEP });
    trace_json_string_field(b"tool", tool);
    trace_json_string_field(b"status", status);
    uart::write_str("}\n");
}

fn trace_fetch_result_snapshot(ok: bool) {
    if !trace_output_enabled() {
        return;
    }
    let status = unsafe { HTTP_STATUS };
    let body_len = unsafe { AGENT_RESPONSE_BODY_LEN };
    let transport = fetch_error_reason();
    trace_begin(b"fetch_result_snapshot", unsafe { M4_LOOP_STEP });
    uart::write_str(",\"ok\":");
    uart::write_str(if ok { "true" } else { "false" });
    uart::write_str(",\"http_status\":");
    uart::write_u64_dec(status as u64);
    uart::write_str(",\"body_len\":");
    uart::write_u64_dec(body_len as u64);
    if !transport.is_empty() {
        trace_json_string_field(b"transport_reason", transport);
    }
    uart::write_str("}\n");
}

fn trace_tool_call_denied(tool: &[u8], reason: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"tool_call_denied", unsafe { M4_LOOP_STEP });
    trace_json_string_field(b"tool", tool);
    trace_json_string_field(b"reason", reason);
    uart::write_str("}\n");
}

fn trace_model_turn_started() {
    trace_event(b"model_turn_started", unsafe { M4_LOOP_STEP });
}

fn trace_model_request_built(body_len: usize) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"model_request_built", unsafe { M4_LOOP_STEP });
    trace_json_u64_field(b"body_len", body_len as u64);
    uart::write_str("}\n");
}

fn trace_model_turn_completed(stop_reason: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"model_turn_completed", unsafe { M4_LOOP_STEP });
    trace_json_string_field(b"stop_reason", stop_reason);
    uart::write_str("}\n");
}

fn trace_assistant_response_rendered() {
    trace_event(b"assistant_response_rendered", unsafe { M4_LOOP_STEP });
}

fn trace_loop_stopped(reason: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"loop_stopped", unsafe { M4_LOOP_STEP });
    trace_json_string_field(b"stop_reason", reason);
    uart::write_str("}\n");
}

fn goal_looks_like_summary_request() -> bool {
    let goal = unsafe { &AGENT_GOAL_TEXT[..AGENT_GOAL_TEXT_LEN] };
    contains_ascii_phrase(goal, goal.len(), b"summarize")
        || contains_ascii_phrase(goal, goal.len(), b"summary")
        || contains_ascii_phrase(goal, goal.len(), b"bullet")
        || contains_ascii_phrase(goal, goal.len(), b"takeaway")
        || contains_ascii_phrase(goal, goal.len(), b"key point")
}

fn compact_fetch_preview(out: &mut [u8], src: &[u8]) -> usize {
    let mut idx = 0usize;
    idx += copy_bytes(&mut out[idx..], b"Fetched URL content preview:\n");
    let body_limit = if src.len() > M4_FETCH_PREVIEW_LIMIT {
        M4_FETCH_PREVIEW_LIMIT
    } else {
        src.len()
    };
    idx += copy_bytes(&mut out[idx..], &src[..body_limit]);
    if src.len() > body_limit {
        idx += copy_bytes(&mut out[idx..], b"\n...(truncated)");
    }
    idx
}

fn append_prompt_section_header(prompt: &mut [u8], prompt_len: &mut usize, title: &[u8]) {
    *prompt_len += copy_bytes(&mut prompt[*prompt_len..], title);
    *prompt_len += copy_bytes(&mut prompt[*prompt_len..], b":\n");
}

fn append_bounded_prompt_bytes(
    prompt: &mut [u8],
    prompt_len: &mut usize,
    src: &[u8],
    limit: usize,
    empty: &[u8],
) {
    let section_end = (*prompt_len + limit).min(prompt.len());
    let take = copy_utf8_prefix(
        &mut prompt[*prompt_len..section_end],
        src,
    );
    if take == 0 {
        *prompt_len += copy_bytes(&mut prompt[*prompt_len..], empty);
    } else {
        *prompt_len += take;
    }
    *prompt_len += copy_bytes(&mut prompt[*prompt_len..], b"\n\n");
}

fn build_m4_openai_request_body(out: &mut [u8]) -> usize {
    const INSTRUCTIONS: &[u8] = b"You are the MiniAgentOS M4 session agent. Available tools: fetch_url(url), post_url(url,json), post_tweet(text), search_recent_posts(query), get_user_posts(username), read_session_state(key), write_session_state(key,value). You must return only compact JSON with no markdown. If you want to call one tool, return one of: {\"type\":\"tool\",\"tool\":\"fetch_url\",\"url\":\"...\"}, {\"type\":\"tool\",\"tool\":\"post_url\",\"url\":\"...\",\"json\":\"{...}\"}, {\"type\":\"tool\",\"tool\":\"post_tweet\",\"text\":\"...\"}, {\"type\":\"tool\",\"tool\":\"search_recent_posts\",\"query\":\"...\"}, {\"type\":\"tool\",\"tool\":\"get_user_posts\",\"username\":\"...\"}, {\"type\":\"tool\",\"tool\":\"read_session_state\",\"key\":\"...\"}, {\"type\":\"tool\",\"tool\":\"write_session_state\",\"key\":\"...\",\"value\":\"...\"}. If you are done, return {\"type\":\"final\",\"response\":\"...\"}. Use at most one tool call per turn. The Current request section is authoritative and always takes precedence over older conversation. Do not continue a previous completed or failed task unless the current request explicitly asks for follow-up. Keep final responses concise and directly answer the user's request; do not append generic follow-up offers unless the user explicitly asks for options. After fetch_url returns content for a summary request, answer with a final response instead of fetching the same URL again unless the user explicitly asks to refetch. Prefer using the Latest tool result section before refetching. For follow-up questions about prior posts or fetched data, prefer read_session_state. For questions about what a specific person or account posted recently, prefer get_user_posts(username). Use search_recent_posts for topic searches, not account timelines. If the user asks for themes, opinions, or takeaways from posts, summarize the main themes from the tool result instead of listing raw JSON. When a search result will matter later, you may use write_session_state. Use post_url for allowed JSON POST requests. If the user asks something outside the available tools, return a brief final refusal.";
    let prompt: &mut [u8] = unsafe { &mut M4_PROMPT_BUF[..] };
    let mut prompt_len = 0usize;
    let current_goal = unsafe { &AGENT_GOAL_TEXT[..AGENT_GOAL_TEXT_LEN] };

    append_prompt_section_header(prompt, &mut prompt_len, b"Current request");
    append_bounded_prompt_bytes(
        prompt,
        &mut prompt_len,
        current_goal,
        M4_CURRENT_REQUEST_LIMIT,
        b"(empty)",
    );

    append_prompt_section_header(prompt, &mut prompt_len, b"Latest tool result");
    unsafe {
        append_bounded_prompt_bytes(
            prompt,
            &mut prompt_len,
            &M4_LAST_TOOL_RESULT[..M4_LAST_TOOL_RESULT_LEN],
            M4_LATEST_TOOL_RESULT_LIMIT,
            b"(none)",
        );
    }

    append_prompt_section_header(prompt, &mut prompt_len, b"Session state");
    let mut state_len = 0usize;
    let state_end = (prompt_len + M4_STATE_SNAPSHOT_LIMIT).min(prompt.len());
    session::append_state_snapshot_to(
        &mut prompt[prompt_len..state_end],
        &mut state_len,
    );
    if state_len == 0 {
        prompt_len += copy_bytes(&mut prompt[prompt_len..], b"(empty)\n\n");
    } else {
        prompt_len += state_len;
        prompt_len += copy_bytes(&mut prompt[prompt_len..], b"\n\n");
    }

    append_prompt_section_header(prompt, &mut prompt_len, b"Recent conversation");
    let mut history_len = 0usize;
    let history_end = (prompt_len + M4_RECENT_CONVERSATION_LIMIT).min(prompt.len());
    session::append_history_suffix_excluding_current_user_to(
        current_goal,
        &mut prompt[prompt_len..history_end],
        &mut history_len,
        M4_RECENT_CONVERSATION_LIMIT,
    );
    if history_len == 0 {
        prompt_len += copy_bytes(&mut prompt[prompt_len..], b"(empty)\n");
    } else {
        prompt_len += history_len;
        if prompt_len == 0 || prompt[prompt_len - 1] != b'\n' {
            prompt_len += copy_bytes(&mut prompt[prompt_len..], b"\n");
        }
    }
    if prompt_len > M4_MODEL_INPUT_LIMIT {
        prompt_len = utf8_safe_prefix_len(&prompt[..prompt_len], M4_MODEL_INPUT_LIMIT);
    }
    let mut idx = 0usize;
    idx = copy_bytes(&mut out[idx..], b"{\"model\":\"") + idx;
    idx = json_escape_append(out, idx, crate::openai::model_name());
    idx = copy_bytes(&mut out[idx..], b"\",\"instructions\":\"") + idx;
    idx = json_escape_append(out, idx, INSTRUCTIONS);
    idx = copy_bytes(&mut out[idx..], b"\",\"input\":\"") + idx;
    idx = json_escape_append(out, idx, &prompt[..prompt_len]);
    idx = copy_bytes(
        &mut out[idx..],
        b"\",\"reasoning\":{\"effort\":\"minimal\"},\"max_output_tokens\":260}",
    ) + idx;
    idx
}

fn start_m4_model_turn() -> bool {
    if !crate::openai::api_key_ready() {
        policy::agent_set_result(b"error", b"missing openai key; run openai-key <key> before M4 input");
        skill::agent_finish_local(unsafe { M4_LOOP_STEP }, AGENT_TERMINAL_FAILED);
        return false;
    }
    m4_prepare_openai_attempt(b"thinking...");
    let body_len = unsafe { build_m4_openai_request_body(&mut FETCH_BODY) };
    let auth_len = unsafe { crate::openai::build_bearer_header(&mut FETCH_EXTRA_HEADER) };
    if body_len == 0 || auth_len == 0 {
        policy::agent_set_result(b"error", b"openai request build failed");
        skill::agent_finish_local(unsafe { M4_LOOP_STEP }, AGENT_TERMINAL_FAILED);
        return false;
    }
    unsafe {
        AGENT_PHASE = AGENT_PHASE_M4_MODEL;
        AGENT_RESPONSE_BODY_LEN = 0;
        AGENT_OUTPUT_TEXT_LEN = 0;
        FETCH_METHOD_POST = true;
        FETCH_BODY_LEN = body_len;
        FETCH_EXTRA_HEADER_LEN = auth_len;
        FETCH_OAUTH_ACTIVE = true;
    }
    trace_model_request_built(body_len);
    trace_model_turn_started();
    human_status(b"thinking...");
    let started =
        skill::fetch_start_agent_url(crate::openai::responses_url(), [10, 0, 2, 15], [0, 0, 0, 0], 0);
    if !started {
        finalize_m4_reason(fetch_failure_reason_or(b"openai loop request failed"));
    }
    started
}

fn start_m4_summary_model_turn() -> bool {
    m4_prepare_openai_attempt(b"summarizing...");
    let body_len = unsafe { model::build_openai_summary_request_body(&mut FETCH_BODY) };
    let auth_len = unsafe { crate::openai::build_bearer_header(&mut FETCH_EXTRA_HEADER) };
    if body_len == 0 || auth_len == 0 {
        finalize_m4_reason(b"openai summary request build failed");
        return false;
    }
    unsafe {
        AGENT_PHASE = AGENT_PHASE_M4_SUMMARY_MODEL;
        AGENT_RESPONSE_BODY_LEN = 0;
        AGENT_OUTPUT_TEXT_LEN = 0;
        FETCH_METHOD_POST = true;
        FETCH_BODY_LEN = body_len;
        FETCH_EXTRA_HEADER_LEN = auth_len;
        FETCH_OAUTH_ACTIVE = true;
    }
    trace_model_request_built(body_len);
    trace_model_turn_started();
    human_status(b"summarizing...");
    let started =
        skill::fetch_start_agent_url(crate::openai::responses_url(), [10, 0, 2, 15], [0, 0, 0, 0], 0);
    if !started {
        finalize_m4_reason(fetch_failure_reason_or(b"openai summary request failed"));
    }
    started
}

fn set_tool_call(tool: &[u8], arg1: &[u8], arg2: &[u8]) {
    unsafe {
        M4_TOOL_NAME_LEN = copy_bytes(&mut M4_TOOL_NAME, tool);
        M4_TOOL_ARG1_LEN = copy_bytes(&mut M4_TOOL_ARG1, arg1);
        M4_TOOL_ARG2_LEN = copy_bytes(&mut M4_TOOL_ARG2, arg2);
    }
}

fn finalize_m4_response(response: &[u8], refused: bool) {
    unsafe {
        AGENT_SUMMARY_LEN = copy_bytes(&mut AGENT_SUMMARY, response);
        AGENT_RESULT_REASON_LEN = 0;
    }
    m4_schedule_user_turn_cooldown();
    session::append_assistant_turn(response);
    clear_inline_status();
    trace_assistant_response_rendered();
    trace_loop_stopped(if refused { b"unsupported" } else { b"final_response" });
    skill::agent_finish_local(
        unsafe { M4_LOOP_STEP },
        if refused {
            AGENT_TERMINAL_REFUSED
        } else {
            AGENT_TERMINAL_COMPLETED
        },
    );
}

fn finalize_m4_reason(reason: &[u8]) {
    policy::agent_set_result(b"error", reason);
    m4_schedule_user_turn_cooldown();
    session::append_assistant_turn(reason);
    clear_inline_status();
    trace_loop_stopped(b"error");
    skill::agent_finish_local(unsafe { M4_LOOP_STEP }, AGENT_TERMINAL_FAILED);
}

fn execute_sync_tool(tool: &[u8]) -> bool {
    let arg1 = unsafe { &M4_TOOL_ARG1[..M4_TOOL_ARG1_LEN] };
    let arg2 = unsafe { &M4_TOOL_ARG2[..M4_TOOL_ARG2_LEN] };
    if starts_with(tool, tool.len(), b"read_session_state") {
        trace_tool_call_with_arg(b"tool_call_requested", tool, b"key", arg1);
        trace_tool_call_with_arg(b"tool_call_started", tool, b"key", arg1);
        let mut value = [0u8; M4_TOOL_RESULT_LIMIT];
        let value_len = session::read_session_state(arg1, &mut value);
        if value_len == 0 {
            let _ = copy_bytes(&mut value, b"(empty)");
        }
        unsafe {
            M4_LAST_TOOL_RESULT_LEN = copy_bytes(&mut M4_LAST_TOOL_RESULT, &value[..if value_len == 0 { 7 } else { value_len }]);
        }
        session::append_tool_result(tool, unsafe {
            &M4_LAST_TOOL_RESULT[..M4_LAST_TOOL_RESULT_LEN]
        });
        trace_tool_call_completed(tool, b"ok");
        unsafe { M4_LOOP_STEP = M4_LOOP_STEP.wrapping_add(1); }
        return start_m4_model_turn();
    }
    if starts_with(tool, tool.len(), b"write_session_state") {
        trace_tool_call_with_two_args(b"tool_call_requested", tool, b"key", arg1, b"value", arg2);
        trace_tool_call_with_two_args(b"tool_call_started", tool, b"key", arg1, b"value", arg2);
        if !session::write_session_state(arg1, arg2) {
            trace_tool_call_completed(tool, b"error");
            finalize_m4_reason(b"session state write failed");
            return false;
        }
        unsafe {
            M4_LAST_TOOL_RESULT_LEN = copy_bytes(&mut M4_LAST_TOOL_RESULT, b"{\"stored\":true}");
        }
        session::append_tool_result(tool, unsafe {
            &M4_LAST_TOOL_RESULT[..M4_LAST_TOOL_RESULT_LEN]
        });
        trace_tool_call_completed(tool, b"ok");
        unsafe { M4_LOOP_STEP = M4_LOOP_STEP.wrapping_add(1); }
        return start_m4_model_turn();
    }
    false
}

fn prepare_get_user_posts(username: &[u8]) -> bool {
    if !oauth::bearer_token_ready() {
        return false;
    }
    let mut query = [0u8; 128];
    let query_len = build_user_posts_query(username, &mut query);
    let path_len = unsafe { build_search_path(&query[..query_len], &mut M4_PATH_BUF) };
    let auth_len = unsafe { oauth::build_bearer_header(&mut FETCH_EXTRA_HEADER) };
    if path_len == 0 || auth_len == 0 {
        return false;
    }
    unsafe {
        FETCH_METHOD_POST = false;
        FETCH_BODY_LEN = 0;
        FETCH_EXTRA_HEADER_LEN = auth_len;
        FETCH_OAUTH_ACTIVE = true;
    }
    fetch_start(XAPI_DOMAIN, unsafe { &M4_PATH_BUF[..path_len] }, [10, 0, 2, 15], [0, 0, 0, 0], 0, true)
}

fn prepare_post_url_body(body: &[u8]) -> bool {
    if body.is_empty() {
        set_fetch_error_reason(b"post_url body missing");
        return false;
    }
    if body.len() > unsafe { FETCH_BODY.len() } {
        set_fetch_error_reason(b"post_url body too large");
        return false;
    }
    unsafe {
        let mut i = 0usize;
        while i < body.len() {
            FETCH_BODY[i] = body[i];
            i += 1;
        }
        FETCH_METHOD_POST = true;
        FETCH_BODY_LEN = body.len();
        FETCH_EXTRA_HEADER_LEN = 0;
        FETCH_OAUTH_ACTIVE = false;
    }
    true
}

fn execute_async_tool(tool: &[u8]) -> bool {
    let arg1 = unsafe { &M4_TOOL_ARG1[..M4_TOOL_ARG1_LEN] };
    let arg2 = unsafe { &M4_TOOL_ARG2[..M4_TOOL_ARG2_LEN] };
    if starts_with(tool, tool.len(), b"fetch_url") {
        trace_tool_call_with_arg(b"tool_call_requested", tool, b"url", arg1);
        trace_tool_call_with_arg(b"tool_call_started", tool, b"url", arg1);
        if unsafe { M4_LAST_FETCH_URL_LEN } != 0
            && unsafe { M4_LAST_FETCH_BODY_LEN } != 0
            && bytes_eq(unsafe { &M4_LAST_FETCH_URL[..M4_LAST_FETCH_URL_LEN] }, arg1)
        {
            unsafe {
                M4_LAST_TOOL_RESULT_LEN =
                    copy_bytes(&mut M4_LAST_TOOL_RESULT, &M4_LAST_FETCH_BODY[..M4_LAST_FETCH_BODY_LEN]);
            }
            session::append_tool_result(tool, unsafe {
                &M4_LAST_TOOL_RESULT[..M4_LAST_TOOL_RESULT_LEN]
            });
            trace_tool_call_completed(tool, b"ok");
            unsafe { M4_LOOP_STEP = M4_LOOP_STEP.wrapping_add(1); }
            return start_m4_model_turn();
        }
        human_status(b"fetching...");
        unsafe {
            AGENT_PHASE = AGENT_PHASE_M4_FETCH_URL;
            AGENT_RESPONSE_BODY_LEN = 0;
            AGENT_OUTPUT_TEXT_LEN = 0;
            FETCH_METHOD_POST = false;
            FETCH_BODY_LEN = 0;
            FETCH_EXTRA_HEADER_LEN = 0;
            FETCH_OAUTH_ACTIVE = false;
        }
        let started = skill::fetch_start_agent_url(arg1, [10, 0, 2, 15], [0, 0, 0, 0], 0);
        if !started {
            finalize_m4_reason(fetch_failure_reason_or(b"fetch_url start failed"));
        }
        return started;
    }
    if starts_with(tool, tool.len(), b"post_url") {
        trace_tool_call_with_two_args(b"tool_call_requested", tool, b"url", arg1, b"json", arg2);
        trace_tool_call_with_two_args(b"tool_call_started", tool, b"url", arg1, b"json", arg2);
        human_status(b"posting...");
        unsafe {
            AGENT_PHASE = AGENT_PHASE_M4_POST_URL;
            AGENT_RESPONSE_BODY_LEN = 0;
            AGENT_OUTPUT_TEXT_LEN = 0;
        }
        let started = prepare_post_url_body(arg2)
            && skill::fetch_start_agent_url(arg1, [10, 0, 2, 15], [0, 0, 0, 0], 0);
        if !started {
            finalize_m4_reason(fetch_failure_reason_or(b"post_url start failed"));
        }
        return started;
    }
    if starts_with(tool, tool.len(), b"post_tweet") {
        trace_tool_call_with_arg(b"tool_call_requested", tool, b"text", arg1);
        if !oauth::secrets_ready() {
            trace_tool_call_denied(tool, b"x oauth secrets missing");
            finalize_m4_reason(b"x oauth secrets missing");
            return false;
        }
        trace_tool_call_with_arg(b"tool_call_started", tool, b"text", arg1);
        human_status(b"posting tweet...");
        unsafe {
            AGENT_PHASE = AGENT_PHASE_M4_POST_TWEET;
            AGENT_RESPONSE_BODY_LEN = 0;
            AGENT_OUTPUT_TEXT_LEN = 0;
        }
        let started = prepare_tweet(arg1)
            && fetch_start(XAPI_DOMAIN, XAPI_PATH, [10, 0, 2, 15], [0, 0, 0, 0], 0, true);
        if !started {
            finalize_m4_reason(fetch_failure_reason_or(b"post_tweet start failed"));
        }
        return started;
    }
    if starts_with(tool, tool.len(), b"search_recent_posts") {
        trace_tool_call_with_arg(b"tool_call_requested", tool, b"query", arg1);
        if !oauth::bearer_token_ready() {
            trace_tool_call_denied(tool, b"x bearer token missing");
            finalize_m4_reason(b"x bearer token missing");
            return false;
        }
        trace_tool_call_with_arg(b"tool_call_started", tool, b"query", arg1);
        human_status(b"searching posts...");
        let path_len = unsafe { build_search_path(arg1, &mut M4_PATH_BUF) };
        let auth_len = unsafe { oauth::build_bearer_header(&mut FETCH_EXTRA_HEADER) };
        if path_len == 0 || auth_len == 0 {
            finalize_m4_reason(b"x search request build failed");
            return false;
        }
        unsafe {
            AGENT_PHASE = AGENT_PHASE_M4_SEARCH_RECENT;
            AGENT_RESPONSE_BODY_LEN = 0;
            AGENT_OUTPUT_TEXT_LEN = 0;
            FETCH_METHOD_POST = false;
            FETCH_BODY_LEN = 0;
            FETCH_EXTRA_HEADER_LEN = auth_len;
            FETCH_OAUTH_ACTIVE = true;
        }
        let started = fetch_start(
            XAPI_DOMAIN,
            unsafe { &M4_PATH_BUF[..path_len] },
            [10, 0, 2, 15],
            [0, 0, 0, 0],
            0,
            true,
        );
        if !started {
            finalize_m4_reason(fetch_failure_reason_or(b"search_recent_posts start failed"));
        }
        return started;
    }
    if starts_with(tool, tool.len(), b"get_user_posts") {
        trace_tool_call_with_arg(b"tool_call_requested", tool, b"username", arg1);
        if !oauth::bearer_token_ready() {
            trace_tool_call_denied(tool, b"x bearer token missing");
            finalize_m4_reason(b"x bearer token missing");
            return false;
        }
        trace_tool_call_with_arg(b"tool_call_started", tool, b"username", arg1);
        human_status(b"loading user posts...");
        unsafe {
            AGENT_PHASE = AGENT_PHASE_M4_GET_USER_POSTS;
            AGENT_RESPONSE_BODY_LEN = 0;
            AGENT_OUTPUT_TEXT_LEN = 0;
        }
        let started = prepare_get_user_posts(arg1);
        if !started {
            finalize_m4_reason(fetch_failure_reason_or(b"get_user_posts start failed"));
        }
        return started;
    }
    false
}

fn parse_m4_tool_from_response(
    response: &[u8],
    tool: &[u8],
    tool_len: usize,
) -> Result<(&'static [u8], bool), &'static [u8]> {
    if starts_with(tool, tool_len, b"fetch_url") {
        let url_len = unsafe { json_extract_string_local(response, b"url", &mut M4_TOOL_ARG1) };
        if url_len == 0 {
            return Err(b"missing fetch url");
        }
        set_tool_call(tool, unsafe { &M4_TOOL_ARG1[..url_len] }, b"");
        return Ok((b"tool", false));
    }
    if starts_with(tool, tool_len, b"post_url") {
        let url_len = unsafe { json_extract_string_local(response, b"url", &mut M4_TOOL_ARG1) };
        let json_len = unsafe { json_extract_string_local(response, b"json", &mut M4_TOOL_ARG2) };
        if url_len == 0 {
            return Err(b"missing post url");
        }
        if json_len == 0 {
            return Err(b"missing post json");
        }
        set_tool_call(
            tool,
            unsafe { &M4_TOOL_ARG1[..url_len] },
            unsafe { &M4_TOOL_ARG2[..json_len] },
        );
        return Ok((b"tool", false));
    }
    if starts_with(tool, tool_len, b"post_tweet") {
        let text_len = unsafe { json_extract_string_local(response, b"text", &mut M4_TOOL_ARG1) };
        if text_len == 0 {
            return Err(b"missing tweet text");
        }
        set_tool_call(tool, unsafe { &M4_TOOL_ARG1[..text_len] }, b"");
        return Ok((b"tool", false));
    }
    if starts_with(tool, tool_len, b"search_recent_posts") {
        let query_len = unsafe { json_extract_string_local(response, b"query", &mut M4_TOOL_ARG1) };
        if query_len == 0 {
            return Err(b"missing search query");
        }
        set_tool_call(tool, unsafe { &M4_TOOL_ARG1[..query_len] }, b"");
        return Ok((b"tool", false));
    }
    if starts_with(tool, tool_len, b"get_user_posts") {
        let username_len =
            unsafe { json_extract_string_local(response, b"username", &mut M4_TOOL_ARG1) };
        if username_len == 0 {
            return Err(b"missing username");
        }
        set_tool_call(tool, unsafe { &M4_TOOL_ARG1[..username_len] }, b"");
        return Ok((b"tool", false));
    }
    if starts_with(tool, tool_len, b"read_session_state") {
        let key_len = unsafe { json_extract_string_local(response, b"key", &mut M4_TOOL_ARG1) };
        if key_len == 0 {
            return Err(b"missing session key");
        }
        set_tool_call(tool, unsafe { &M4_TOOL_ARG1[..key_len] }, b"");
        return Ok((b"tool", false));
    }
    if starts_with(tool, tool_len, b"write_session_state") {
        let key_len = unsafe { json_extract_string_local(response, b"key", &mut M4_TOOL_ARG1) };
        let value_len =
            unsafe { json_extract_string_local(response, b"value", &mut M4_TOOL_ARG2) };
        if key_len == 0 {
            return Err(b"missing session key");
        }
        set_tool_call(
            tool,
            unsafe { &M4_TOOL_ARG1[..key_len] },
            unsafe { &M4_TOOL_ARG2[..value_len] },
        );
        return Ok((b"tool", false));
    }
    Err(b"unsupported tool")
}

fn parse_m4_final_from_response(response: &[u8]) -> Result<(&'static [u8], bool), &'static [u8]> {
    let response_len =
        unsafe { json_extract_string_local(response, b"response", &mut M4_LAST_TOOL_RESULT) };
    if response_len == 0 {
        let partial_len =
            unsafe { json_extract_string_partial_local(response, b"response", &mut M4_LAST_TOOL_RESULT) };
        if partial_len != 0 {
            unsafe {
                M4_LAST_TOOL_RESULT_LEN = partial_len;
            }
        } else {
            let trimmed_len = copy_trimmed_text(unsafe { &mut M4_LAST_TOOL_RESULT }, response);
            if trimmed_len == 0 {
                return Err(b"missing final response");
            }
            unsafe {
                M4_LAST_TOOL_RESULT_LEN = trimmed_len;
            }
        }
    } else {
        unsafe {
            M4_LAST_TOOL_RESULT_LEN = response_len;
        }
    }
    Ok((
        b"final",
        starts_with(
            unsafe { &M4_LAST_TOOL_RESULT[..M4_LAST_TOOL_RESULT_LEN] },
            unsafe { M4_LAST_TOOL_RESULT_LEN },
            b"unsupported goal",
        ),
    ))
}

fn parse_m4_model_response() -> Result<(&'static [u8], bool), &'static [u8]> {
    let len = match unsafe { super::extract_openai_output_text(&mut M4_MODEL_TEXT_BUF) } {
        Some(v) if v != 0 => v,
        _ => return Err(b"empty model response"),
    };
    let response = unsafe { &M4_MODEL_TEXT_BUF[..len] };
    let mut kind = [0u8; 16];
    let kind_len = json_extract_string_local(response, b"type", &mut kind);
    if kind_len != 0 {
        if starts_with(&kind[..], kind_len, b"final") {
            return parse_m4_final_from_response(response);
        }
        if !starts_with(&kind[..], kind_len, b"tool") {
            return Err(b"unsupported model response");
        }
        let mut tool = [0u8; 32];
        let tool_len = json_extract_string_local(response, b"tool", &mut tool);
        if tool_len == 0 {
            return Err(b"missing tool");
        }
        return parse_m4_tool_from_response(response, &tool[..tool_len], tool_len);
    }

    let mut tool = [0u8; 32];
    let tool_len = json_extract_string_local(response, b"tool", &mut tool);
    if tool_len != 0 {
        return parse_m4_tool_from_response(response, &tool[..tool_len], tool_len);
    }

    let mut response_buf = [0u8; 1024];
    if json_extract_string_local(response, b"response", &mut response_buf) != 0 {
        return parse_m4_final_from_response(response);
    }

    parse_m4_final_from_response(response)
}

fn complete_tool_and_continue(status: &[u8], auto_store_key: Option<&[u8]>) -> bool {
    let tool = unsafe { &M4_TOOL_NAME[..M4_TOOL_NAME_LEN] };
    trace_tool_call_completed(tool, status);
    if starts_with(status, status.len(), b"ok") {
        if let Some(key) = auto_store_key {
            let value = unsafe { &M4_LAST_TOOL_RESULT[..M4_LAST_TOOL_RESULT_LEN] };
            let _ = session::write_session_state(key, value);
        }
        session::append_tool_result(tool, unsafe {
            &M4_LAST_TOOL_RESULT[..M4_LAST_TOOL_RESULT_LEN]
        });
        unsafe { M4_LOOP_STEP = M4_LOOP_STEP.wrapping_add(1); }
        if unsafe { M4_LOOP_STEP } > M4_LOOP_MAX_STEPS {
            finalize_m4_reason(b"m4 loop budget exceeded");
            return true;
        }
        return start_m4_model_turn();
    }
    finalize_m4_reason(unsafe { &M4_LAST_TOOL_RESULT[..M4_LAST_TOOL_RESULT_LEN] });
    true
}

fn capture_raw_response_body() -> usize {
    let src = unsafe { &AGENT_RESPONSE_BODY[..AGENT_RESPONSE_BODY_LEN] };
    copy_trimmed_text(unsafe { &mut M4_LAST_TOOL_RESULT }, src)
}

pub(crate) fn handle_m4_goal_line(line: &[u8], len: usize) -> bool {
    if !is_m4_candidate(line, len) {
        return false;
    }
    if unsafe { AGENT_TASK_ACTIVE || FETCH_STATE != FETCH_IDLE } {
        uart::write_str("busy\n");
        return true;
    }
    let mut start = 0usize;
    if starts_with(&line[..], len, b"m4 ") {
        start = 3;
        while start < len && is_space(line[start]) {
            start += 1;
        }
    }
    let goal = if start < len { &line[start..len] } else { &line[..len] };
    m4_apply_user_turn_cooldown();
    session::ensure_session_started();
    m4_reset_turn_state();
    unsafe {
        AGENT_TASK_ACTIVE = true;
        AGENT_MODE = AGENT_MODE_M4;
        AGENT_PHASE = AGENT_PHASE_M4_MODEL;
        AGENT_GOAL_ID_LEN = copy_bytes(&mut AGENT_GOAL_ID, b"m4-session");
        AGENT_GOAL_TEXT_LEN = copy_bytes(&mut AGENT_GOAL_TEXT, goal);
        AGENT_RESPONSE_BODY_LEN = 0;
        AGENT_OUTPUT_TEXT_LEN = 0;
        AGENT_SUMMARY_LEN = 0;
        AGENT_RESULT_REASON_LEN = 0;
        AGENT_RESULT_STATUS_LEN = 0;
        M4_LOOP_STEP = 0;
    }
    trace_event(b"user_turn_received", 0);
    session::append_user_turn(goal);
    if try_handle_local_meta_request(goal) {
        return true;
    }
    start_m4_model_turn();
    true
}

pub(crate) fn handle_m4_fetch_done(ok: bool) -> bool {
    if unsafe { AGENT_MODE } != AGENT_MODE_M4 {
        return false;
    }
    let phase = unsafe { AGENT_PHASE };
    trace_fetch_result_snapshot(ok);
    if phase == AGENT_PHASE_M4_MODEL {
        if !model::agent_http_success(ok) {
            if model::openai_failure_retryable()
                && unsafe { M4_MODEL_RETRIES } < M4_OPENAI_MAX_RETRIES
            {
                unsafe {
                    M4_MODEL_RETRIES = M4_MODEL_RETRIES.wrapping_add(1);
                }
                trace_retry_scheduled(unsafe { M4_LOOP_STEP }, b"m4_model", unsafe {
                    M4_MODEL_RETRIES
                });
                human_status(b"retrying...");
                return start_m4_model_turn();
            }
            if model::openai_failure_retryable() {
                m4_mark_retryable_openai_failure();
            }
            let mut reason = [0u8; 160];
            let reason_len = model::build_openai_failure_reason(&mut reason, b"openai loop");
            finalize_m4_reason(&reason[..reason_len]);
            return true;
        }
        match parse_m4_model_response() {
            Ok((kind, refused)) if starts_with(kind, kind.len(), b"final") => {
                trace_model_turn_completed(b"final_response");
                finalize_m4_response(
                    unsafe { &M4_LAST_TOOL_RESULT[..M4_LAST_TOOL_RESULT_LEN] },
                    refused,
                );
            }
            Ok((kind, _)) if starts_with(kind, kind.len(), b"tool") => {
                trace_model_turn_completed(b"tool_call");
                let tool = unsafe { &M4_TOOL_NAME[..M4_TOOL_NAME_LEN] };
                if execute_sync_tool(tool) {
                    return true;
                }
                if !execute_async_tool(tool) {
                    return true;
                }
            }
            _ => finalize_m4_reason(b"invalid model response"),
        }
        return true;
    }

    if phase == AGENT_PHASE_M4_SUMMARY_MODEL {
        if model::agent_http_success(ok)
            && model::agent_response_body_truncated()
            && !model::cached_openai_output_text_ready()
        {
            finalize_m4_reason(b"openai summary: response body truncated");
            return true;
        }
        if !model::agent_http_success(ok) {
            if model::openai_failure_retryable()
                && unsafe { M4_MODEL_RETRIES } < M4_OPENAI_MAX_RETRIES
            {
                unsafe {
                    M4_MODEL_RETRIES = M4_MODEL_RETRIES.wrapping_add(1);
                }
                trace_retry_scheduled(unsafe { M4_LOOP_STEP }, b"m4_summary", unsafe {
                    M4_MODEL_RETRIES
                });
                human_status(b"retrying summary...");
                return start_m4_summary_model_turn();
            }
            if model::openai_failure_retryable() {
                m4_mark_retryable_openai_failure();
            }
            let mut reason = [0u8; 160];
            let reason_len = model::build_openai_failure_reason(&mut reason, b"openai summary");
            finalize_m4_reason(&reason[..reason_len]);
            return true;
        }
        if !model::capture_openai_summary() {
            finalize_m4_reason(b"invalid summary response");
            return true;
        }
        trace_model_turn_completed(b"final_response");
        finalize_m4_response(unsafe { &AGENT_SUMMARY[..AGENT_SUMMARY_LEN] }, false);
        return true;
    }

    let tool = unsafe { &M4_TOOL_NAME[..M4_TOOL_NAME_LEN] };
    let response_truncated = model::agent_response_body_truncated();
    let ok_status = model::agent_http_success(ok) && !response_truncated;
    unsafe {
        M4_LAST_TOOL_RESULT_LEN = if ok_status {
            capture_raw_response_body()
        } else if model::agent_http_success(ok) && response_truncated {
            let phase: &[u8] = if starts_with(tool, tool.len(), b"post_url") {
                b"post_url"
            } else if starts_with(tool, tool.len(), b"post_tweet") {
                b"post_tweet"
            } else if starts_with(tool, tool.len(), b"search_recent_posts") {
                b"search_recent_posts"
            } else if starts_with(tool, tool.len(), b"get_user_posts") {
                b"get_user_posts"
            } else {
                b"fetch_url"
            };
            let mut reason = [0u8; 160];
            let mut len = copy_bytes(&mut reason, phase);
            len += copy_bytes(&mut reason[len..], b": response body truncated");
            copy_bytes(&mut M4_LAST_TOOL_RESULT, &reason[..len])
        } else {
            let mut reason = [0u8; 160];
            let len = if starts_with(tool, tool.len(), b"post_url") {
                model::build_openai_failure_reason(&mut reason, b"post_url")
            } else if starts_with(tool, tool.len(), b"post_tweet") {
                model::build_openai_failure_reason(&mut reason, b"post_tweet")
            } else if starts_with(tool, tool.len(), b"search_recent_posts") {
                model::build_openai_failure_reason(&mut reason, b"search_recent_posts")
            } else if starts_with(tool, tool.len(), b"get_user_posts") {
                model::build_openai_failure_reason(&mut reason, b"get_user_posts")
            } else {
                model::build_openai_failure_reason(&mut reason, b"fetch_url")
            };
            copy_bytes(&mut M4_LAST_TOOL_RESULT, &reason[..len])
        };
    }
    if phase == AGENT_PHASE_M4_POST_URL && ok_status && unsafe { M4_LAST_TOOL_RESULT_LEN } == 0 {
        unsafe {
            M4_LAST_TOOL_RESULT_LEN = copy_bytes(&mut M4_LAST_TOOL_RESULT, b"{\"ok\":true}");
        }
    }
    if phase == AGENT_PHASE_M4_SEARCH_RECENT || phase == AGENT_PHASE_M4_GET_USER_POSTS {
        return complete_tool_and_continue(if ok_status { b"ok" } else { b"error" }, Some(b"last_posts"));
    }
    if phase == AGENT_PHASE_M4_FETCH_URL && ok_status {
        let preview_len = unsafe {
            compact_fetch_preview(
                &mut M4_LAST_FETCH_BODY,
                &M4_LAST_TOOL_RESULT[..M4_LAST_TOOL_RESULT_LEN],
            )
        };
        unsafe {
            M4_LAST_FETCH_URL_LEN =
                copy_bytes(&mut M4_LAST_FETCH_URL, &M4_TOOL_ARG1[..M4_TOOL_ARG1_LEN]);
            M4_LAST_FETCH_BODY_LEN = preview_len;
            M4_LAST_TOOL_RESULT_LEN =
                copy_bytes(&mut M4_LAST_TOOL_RESULT, &M4_LAST_FETCH_BODY[..preview_len]);
        }
        let _ = session::write_session_state(
            b"last_fetch_url",
            unsafe { &M4_TOOL_ARG1[..M4_TOOL_ARG1_LEN] },
        );
        if goal_looks_like_summary_request() {
            trace_tool_call_completed(tool, b"ok");
            session::append_tool_result(tool, unsafe {
                &M4_LAST_TOOL_RESULT[..M4_LAST_TOOL_RESULT_LEN]
            });
            unsafe {
                M4_MODEL_RETRIES = 0;
                M4_LOOP_STEP = M4_LOOP_STEP.wrapping_add(1);
            }
            return start_m4_summary_model_turn();
        }
    }
    complete_tool_and_continue(if ok_status { b"ok" } else { b"error" }, None)
}
