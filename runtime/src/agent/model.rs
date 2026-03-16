use super::*;

pub(crate) fn agent_capture_response_body(data: &[u8]) {
    unsafe {
        let phase = AGENT_PHASE;
        let capture_phase = AGENT_TASK_ACTIVE
            && (phase == AGENT_PHASE_FETCH_SOURCE
                || phase == AGENT_PHASE_CALL_MODEL
                || phase == AGENT_PHASE_INTERPRET_GOAL
                || phase == AGENT_PHASE_M4_MODEL
                || phase == AGENT_PHASE_M4_FETCH_URL
                || phase == AGENT_PHASE_M4_POST_URL
                || phase == AGENT_PHASE_M4_POST_TWEET
                || phase == AGENT_PHASE_M4_SEARCH_RECENT
                || phase == AGENT_PHASE_M4_GET_USER_POSTS
                || phase == AGENT_PHASE_M4_SUMMARY_MODEL);
        if !capture_phase {
            return;
        }
        let openai_phase = phase == AGENT_PHASE_CALL_MODEL
            || phase == AGENT_PHASE_INTERPRET_GOAL
            || phase == AGENT_PHASE_M4_MODEL
            || phase == AGENT_PHASE_M4_SUMMARY_MODEL;
        if AGENT_RESPONSE_BODY_LEN == 0 {
            AGENT_RESPONSE_BODY_TRUNCATED = false;
            AGENT_OUTPUT_TEXT_LEN = 0;
        }
        if openai_phase
            && AGENT_OUTPUT_TEXT_LEN != 0
            && HTTP_STATUS >= 200
            && HTTP_STATUS < 300
        {
            return;
        }
        let remain = AGENT_RESPONSE_BODY.len().saturating_sub(AGENT_RESPONSE_BODY_LEN);
        let mut take = data.len();
        if take > remain {
            take = remain;
            AGENT_RESPONSE_BODY_TRUNCATED = true;
        }
        let mut i = 0usize;
        while i < take {
            AGENT_RESPONSE_BODY[AGENT_RESPONSE_BODY_LEN + i] = data[i];
            i += 1;
        }
        AGENT_RESPONSE_BODY_LEN += take;
        if openai_phase && HTTP_STATUS >= 200 && HTTP_STATUS < 300 {
            if let Some(len) = extract_openai_nested_output_text(&mut AGENT_OUTPUT_TEXT) {
                if len != 0 {
                    AGENT_OUTPUT_TEXT_LEN = len;
                }
            }
        }
    }
}

pub(crate) fn agent_response_body_truncated() -> bool {
    unsafe { AGENT_RESPONSE_BODY_TRUNCATED }
}

pub(crate) fn cached_openai_output_text_ready() -> bool {
    unsafe { AGENT_OUTPUT_TEXT_LEN != 0 }
}

pub(super) fn build_goal_interpretation_request_body(out: &mut [u8]) -> usize {
    let mut i = 0usize;
    let head = b"{\"goal_text\":\"";
    let tail = b"\"}";
    i = copy_bytes(&mut out[i..], head) + i;
    i = json_escape_append(out, i, unsafe { &AGENT_GOAL_TEXT[..AGENT_GOAL_TEXT_LEN] });
    i = copy_bytes(&mut out[i..], tail) + i;
    i
}

fn trim_ascii_bounds(buf: &[u8], len: usize) -> (usize, usize) {
    let mut start = 0usize;
    let mut end = len;
    while start < end && is_space(buf[start]) {
        start += 1;
    }
    while end > start && is_space(buf[end - 1]) {
        end -= 1;
    }
    (start, end)
}

fn json_find_key_from(buf: &[u8], len: usize, key: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i + key.len() + 2 < len {
        if buf[i] == b'"' {
            let mut ok = true;
            let mut j = 0usize;
            while j < key.len() {
                if buf[i + 1 + j] != key[j] {
                    ok = false;
                    break;
                }
                j += 1;
            }
            if ok && buf[i + 1 + key.len()] == b'"' {
                let mut off = i + key.len() + 2;
                while off < len && is_space(buf[off]) {
                    off += 1;
                }
                if off < len && buf[off] == b':' {
                    return Some(off + 1);
                }
            }
        }
        i += 1;
    }
    None
}

fn json_extract_string_from(
    buf: &[u8],
    len: usize,
    key: &[u8],
    out: &mut [u8],
    start: usize,
) -> Option<(usize, usize)> {
    let mut off = json_find_key_from(buf, len, key, start)?;
    while off < len && is_space(buf[off]) {
        off += 1;
    }
    if off >= len || buf[off] != b'"' {
        return None;
    }
    let value_start = off;
    off += 1;
    let mut n = 0usize;
    while off < len {
        let b = buf[off];
        if b == b'"' {
            return Some((value_start, n));
        }
        let mut v = b;
        if b == b'\\' {
            off += 1;
            if off >= len {
                return None;
            }
            if buf[off] == b'u' {
                if off + 4 >= len {
                    return None;
                }
                let d0 = hex_nibble(buf[off + 1])?;
                let d1 = hex_nibble(buf[off + 2])?;
                let d2 = hex_nibble(buf[off + 3])?;
                let d3 = hex_nibble(buf[off + 4])?;
                let cp = ((d0 as u32) << 12)
                    | ((d1 as u32) << 8)
                    | ((d2 as u32) << 4)
                    | (d3 as u32);
                append_utf8_codepoint(out, &mut n, cp);
                off += 5;
                continue;
            }
            v = match buf[off] {
                b'n' => b'\n',
                b'r' => b'\r',
                b't' => b'\t',
                b'"' => b'"',
                b'\\' => b'\\',
                other => other,
            };
        }
        if n < out.len() {
            out[n] = v;
            n += 1;
        }
        off += 1;
    }
    None
}

fn extract_openai_nested_output_text(out: &mut [u8]) -> Option<usize> {
    let body = unsafe { &AGENT_RESPONSE_BODY[..AGENT_RESPONSE_BODY_LEN] };
    let len = body.len();
    let mut scan = 0usize;
    let mut kind = [0u8; 32];
    while scan < len {
        let (value_start, value_len) = json_extract_string_from(body, len, b"type", &mut kind, scan)?;
        if starts_with(&kind[..], value_len, b"output_text") {
            let (_, text_len) = json_extract_string_from(body, len, b"text", out, value_start)?;
            let (start, end) = trim_ascii_bounds(out, text_len);
            let mut n = end.saturating_sub(start);
            if n > out.len() {
                n = out.len();
            }
            if start != 0 {
                let mut i = 0usize;
                while i < n {
                    out[i] = out[start + i];
                    i += 1;
                }
            }
            return Some(n);
        }
        scan = value_start.saturating_add(value_len).saturating_add(1);
    }
    None
}

pub(crate) fn extract_openai_output_text(out: &mut [u8]) -> Option<usize> {
    unsafe {
        if AGENT_OUTPUT_TEXT_LEN != 0 {
            let mut n = AGENT_OUTPUT_TEXT_LEN;
            if n > out.len() {
                n = out.len();
            }
            let mut i = 0usize;
            while i < n {
                out[i] = AGENT_OUTPUT_TEXT[i];
                i += 1;
            }
            return Some(n);
        }
    }
    if let Some(len) = extract_openai_nested_output_text(out) {
        return Some(len);
    }
    let len = unsafe {
        json_extract_string(
            &AGENT_RESPONSE_BODY[..AGENT_RESPONSE_BODY_LEN],
            AGENT_RESPONSE_BODY_LEN,
            b"output_text",
            out,
        )
    }?;
    let (start, end) = trim_ascii_bounds(out, len);
    let mut n = end.saturating_sub(start);
    if n > out.len() {
        n = out.len();
    }
    if start != 0 {
        let mut i = 0usize;
        while i < n {
            out[i] = out[start + i];
            i += 1;
        }
    }
    Some(n)
}

pub(super) fn build_openai_interpretation_request_body(out: &mut [u8]) -> usize {
    const INSTRUCTIONS: &[u8] = b"You are the in-guest goal interpreter for MiniAgentOS. Supported goals are limited to two families: (1) summarize one URL directly for the user, (2) summarize one URL and post the result to one sink URL. Return only compact JSON. Direct summary => {\"status\":\"ok\",\"action\":\"local_summary\",\"source_url\":\"...\",\"max_items\":3,\"output_language\":\"zh\",\"style\":\"bullet\"}. Posted summary => {\"status\":\"ok\",\"action\":\"post_summary\",\"source_url\":\"...\",\"sink_url\":\"...\",\"max_items\":3,\"output_language\":\"zh\",\"style\":\"bullet\"}. If no explicit output language is requested, use \"default\". If unsupported => {\"status\":\"error\",\"reason\":\"unsupported goal\"}. Treat bullet point takeaways and key points as summary requests. Preserve explicit output language and output style requests as structured fields. If the user gives a bare domain or host without a scheme, normalize it to an https:// URL. Tolerate minor typos such as 'summury' when the intent is clearly to summarize.";
    let mut i = 0usize;
    i = copy_bytes(&mut out[i..], b"{\"model\":\"") + i;
    i = json_escape_append(out, i, crate::openai::model_name());
    i = copy_bytes(&mut out[i..], b"\",\"instructions\":\"") + i;
    i = json_escape_append(out, i, INSTRUCTIONS);
    i = copy_bytes(&mut out[i..], b"\",\"input\":\"") + i;
    i = json_escape_append(out, i, unsafe { &AGENT_GOAL_TEXT[..AGENT_GOAL_TEXT_LEN] });
    i = copy_bytes(
        &mut out[i..],
        b"\",\"reasoning\":{\"effort\":\"minimal\"},\"max_output_tokens\":240}",
    ) + i;
    i
}

pub(crate) fn agent_http_success(ok: bool) -> bool {
    ok && unsafe { HTTP_STATUS >= 200 && HTTP_STATUS < 300 }
}

fn append_reason_part(out: &mut [u8], idx: &mut usize, part: &[u8]) {
    *idx += copy_bytes(&mut out[*idx..], part);
}

fn append_http_status(out: &mut [u8], idx: &mut usize, status: u16) {
    append_u64_dec(out, idx, status as u64);
}

fn copy_trimmed(dst: &mut [u8], src: &[u8], len: usize) -> usize {
    let (start, end) = trim_ascii_bounds(src, len);
    if end <= start {
        return 0;
    }
    copy_bytes(dst, &src[start..end])
}

pub(crate) fn build_openai_failure_reason(out: &mut [u8], phase: &[u8]) -> usize {
    let mut idx = 0usize;
    append_reason_part(out, &mut idx, phase);
    append_reason_part(out, &mut idx, b": ");

    let status = unsafe { HTTP_STATUS };
    let transport = fetch_error_reason();
    if status == 0 {
        if !transport.is_empty() {
            append_reason_part(out, &mut idx, transport);
        } else {
            append_reason_part(out, &mut idx, b"request failed");
        }
        return idx;
    }

    if status >= 200 && status < 300 {
        if agent_response_body_truncated() {
            append_reason_part(out, &mut idx, b"response body truncated");
        } else {
            append_reason_part(out, &mut idx, b"invalid response format");
        }
        return idx;
    }

    if !transport.is_empty() && status >= 300 && status < 400 {
        append_reason_part(out, &mut idx, transport);
        return idx;
    }

    let body = unsafe { &AGENT_RESPONSE_BODY[..AGENT_RESPONSE_BODY_LEN] };
    let body_len = unsafe { AGENT_RESPONSE_BODY_LEN };
    let mut raw_message = [0u8; 112];
    let mut message = [0u8; 112];
    let mut raw_code = [0u8; 40];
    let mut code = [0u8; 40];
    let message_len = match json_extract_string(body, body_len, b"message", &mut raw_message) {
        Some(len) => copy_trimmed(&mut message, &raw_message, len),
        None => 0,
    };
    let code_len = match json_extract_string(body, body_len, b"code", &mut raw_code) {
        Some(len) => copy_trimmed(&mut code, &raw_code, len),
        None => 0,
    };

    match status {
        400 => append_reason_part(out, &mut idx, b"bad request"),
        401 => append_reason_part(out, &mut idx, b"unauthorized"),
        403 => append_reason_part(out, &mut idx, b"forbidden"),
        404 => append_reason_part(out, &mut idx, b"not found"),
        408 => append_reason_part(out, &mut idx, b"request timeout"),
        409 => append_reason_part(out, &mut idx, b"conflict"),
        422 => append_reason_part(out, &mut idx, b"unprocessable request"),
        429 => append_reason_part(out, &mut idx, b"rate limited"),
        500..=599 => append_reason_part(out, &mut idx, b"server error"),
        _ => append_reason_part(out, &mut idx, b"http error"),
    }

    append_reason_part(out, &mut idx, b" (");
    append_http_status(out, &mut idx, status);
    append_reason_part(out, &mut idx, b")");

    let terse_only = matches!(status, 401 | 403 | 429) || (500..=599).contains(&status);
    if code_len != 0 {
        append_reason_part(out, &mut idx, b" ");
        append_reason_part(out, &mut idx, &code[..code_len]);
    }
    if message_len != 0 && !terse_only {
        let mut shown = message_len;
        let max_message = 72usize;
        if shown > max_message {
            shown = max_message;
            while shown > 0 && message[shown - 1] != b' ' && message[shown - 1] != b',' {
                shown -= 1;
            }
            if shown < 32 {
                shown = max_message;
            }
        }
        append_reason_part(out, &mut idx, b": ");
        append_reason_part(out, &mut idx, &message[..shown]);
        if shown < message_len {
            append_reason_part(out, &mut idx, b"...");
        }
    }

    idx
}

pub(crate) fn openai_failure_retryable() -> bool {
    let status = unsafe { HTTP_STATUS };
    if status == 429 || (500..=599).contains(&status) {
        return true;
    }
    if status != 0 {
        return false;
    }
    let reason = fetch_error_reason();
    starts_with(reason, reason.len(), b"network request timed out")
        || starts_with(reason, reason.len(), b"gateway arp timed out")
        || starts_with(reason, reason.len(), b"dns lookup timed out")
        || starts_with(reason, reason.len(), b"tcp connect timed out")
        || starts_with(reason, reason.len(), b"proxy handshake timed out")
        || starts_with(reason, reason.len(), b"proxy connect timed out")
        || starts_with(reason, reason.len(), b"http response timed out")
        || starts_with(reason, reason.len(), b"tls handshake failed")
        || starts_with(reason, reason.len(), b"tls write failed")
        || starts_with(reason, reason.len(), b"tls read failed")
}

pub(super) fn summarize_agent_response(out: &mut [u8], sentence_limit: usize) -> usize {
    let limit = if sentence_limit == 0 { 1 } else { sentence_limit };
    let src = unsafe { &AGENT_RESPONSE_BODY[..AGENT_RESPONSE_BODY_LEN] };
    let mut out_len = 0usize;
    let mut saw_space = true;
    let mut sentence_count = 0usize;
    let mut i = 0usize;
    while i < src.len() && out_len < out.len() {
        let mut b = src[i];
        if b == b'\n' || b == b'\r' || b == b'\t' {
            b = b' ';
        }
        if b == b' ' {
            if !saw_space && out_len < out.len() {
                out[out_len] = b;
                out_len += 1;
                saw_space = true;
            }
            i += 1;
            continue;
        }
        out[out_len] = b;
        out_len += 1;
        saw_space = false;
        if b == b'.' || b == b'!' || b == b'?' {
            sentence_count += 1;
            if sentence_count >= limit {
                break;
            }
        }
        i += 1;
    }
    while out_len > 0 && out[out_len - 1] == b' ' {
        out_len -= 1;
    }
    out_len
}

pub(super) fn build_agent_result_body(out: &mut [u8]) -> usize {
    let mut i = 0usize;
    let head = b"{\"goal_id\":\"";
    let mid = b"\",\"status\":\"";
    let summary_head = b"\",\"summary\":\"";
    let reason_head = b"\",\"reason\":\"";
    let tail = b"\"}";
    i = copy_bytes(&mut out[i..], head) + i;
    i = json_escape_append(out, i, unsafe { &AGENT_GOAL_ID[..AGENT_GOAL_ID_LEN] });
    i = copy_bytes(&mut out[i..], mid) + i;
    i = json_escape_append(out, i, unsafe { &AGENT_RESULT_STATUS[..AGENT_RESULT_STATUS_LEN] });
    if unsafe { AGENT_SUMMARY_LEN } > 0 {
        i = copy_bytes(&mut out[i..], summary_head) + i;
        i = json_escape_append(out, i, unsafe { &AGENT_SUMMARY[..AGENT_SUMMARY_LEN] });
    }
    if unsafe { AGENT_RESULT_REASON_LEN } > 0 {
        i = copy_bytes(&mut out[i..], reason_head) + i;
        i = json_escape_append(out, i, unsafe { &AGENT_RESULT_REASON[..AGENT_RESULT_REASON_LEN] });
    }
    i = copy_bytes(&mut out[i..], tail) + i;
    i
}

pub(super) fn build_model_request_body(out: &mut [u8]) -> usize {
    let mut i = 0usize;
    let head = b"{\"source_text\":\"";
    let mid = b"\",\"max_items\":";
    let tail = b"}";
    i = copy_bytes(&mut out[i..], head) + i;
    i = json_escape_append(out, i, unsafe { &AGENT_RESPONSE_BODY[..AGENT_RESPONSE_BODY_LEN] });
    i = copy_bytes(&mut out[i..], mid) + i;
    let max_items = unsafe { AGENT_SUMMARY_SENTENCES as u64 };
    append_u64_dec(out, &mut i, max_items);
    i = copy_bytes(&mut out[i..], tail) + i;
    i
}

pub(super) fn build_openai_summary_request_body(out: &mut [u8]) -> usize {
    const SOURCE_LIMIT: usize = 2200;
    let mut i = 0usize;
    i = copy_bytes(&mut out[i..], b"{\"model\":\"") + i;
    i = json_escape_append(out, i, crate::openai::model_name());
    i = copy_bytes(&mut out[i..], b"\",\"instructions\":\"") + i;
    i = copy_bytes(
        &mut out[i..],
        b"You are the in-guest summary engine for MiniAgentOS. Summarize the provided source into at most ",
    ) + i;
    append_u64_dec(out, &mut i, unsafe { AGENT_SUMMARY_SENTENCES as u64 });
    i = copy_bytes(
        &mut out[i..],
        b" concise bullet points. Use the structured output language and style hints provided below. Return only the final summary as plain text.\",\"input\":\"",
    ) + i;
    i = copy_bytes(&mut out[i..], b"Output language: ") + i;
    let output_language = unsafe {
        if AGENT_OUTPUT_LANGUAGE_LEN > 0 {
            &AGENT_OUTPUT_LANGUAGE[..AGENT_OUTPUT_LANGUAGE_LEN]
        } else {
            b"default".as_slice()
        }
    };
    i = json_escape_append(out, i, output_language);
    i = copy_bytes(&mut out[i..], b"\\nOutput style: ") + i;
    let output_style = unsafe {
        if AGENT_OUTPUT_STYLE_LEN > 0 {
            &AGENT_OUTPUT_STYLE[..AGENT_OUTPUT_STYLE_LEN]
        } else {
            b"bullet".as_slice()
        }
    };
    i = json_escape_append(out, i, output_style);
    i = copy_bytes(&mut out[i..], b"\\nOriginal goal: ") + i;
    i = json_escape_append(out, i, unsafe { &AGENT_GOAL_TEXT[..AGENT_GOAL_TEXT_LEN] });
    i = copy_bytes(&mut out[i..], b"\\n\\nSource:\\n") + i;
    let src = unsafe {
        if AGENT_RESPONSE_BODY_LEN > SOURCE_LIMIT {
            &AGENT_RESPONSE_BODY[..SOURCE_LIMIT]
        } else {
            &AGENT_RESPONSE_BODY[..AGENT_RESPONSE_BODY_LEN]
        }
    };
    i = json_escape_append(out, i, src);
    i = copy_bytes(
        &mut out[i..],
        b"\",\"reasoning\":{\"effort\":\"minimal\"},\"max_output_tokens\":220}",
    ) + i;
    i
}

pub(super) fn capture_model_summary() -> bool {
    let len = unsafe {
        json_extract_string(
            &AGENT_RESPONSE_BODY[..AGENT_RESPONSE_BODY_LEN],
            AGENT_RESPONSE_BODY_LEN,
            b"summary",
            &mut AGENT_SUMMARY,
        )
    };
    match len {
        Some(v) => {
            unsafe {
                AGENT_SUMMARY_LEN = v;
            }
            true
        }
        None => false,
    }
}

pub(super) fn capture_openai_summary() -> bool {
    let len = unsafe { extract_openai_output_text(&mut AGENT_SUMMARY) };
    match len {
        Some(v) if v != 0 => {
            unsafe {
                AGENT_SUMMARY_LEN = v;
            }
            true
        }
        _ => false,
    }
}
