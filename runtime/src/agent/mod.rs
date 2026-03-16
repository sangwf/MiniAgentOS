use super::*;

mod goal;
mod r#loop;
mod model;
mod policy;
mod session;
mod skill;
mod task;

pub(crate) use goal::handle_goal_line;
pub(crate) use model::agent_capture_response_body;
pub(crate) use r#loop::{handle_m4_fetch_done, handle_m4_goal_line, handle_session_command};
pub(crate) use skill::handle_agent_fetch_done;
pub(crate) use task::handle_agent_task_line;
pub(super) use model::extract_openai_output_text;

fn current_trace_step() -> u8 {
    unsafe {
        if AGENT_MODE == AGENT_MODE_M4 {
            r#loop::current_loop_step()
        } else {
            0
        }
    }
}

fn trace_begin(event: &[u8], step: u8) {
    if !trace_output_enabled() {
        return;
    }
    uart::write_str("TRACE {\"event\":\"");
    uart::write_bytes(event);
    uart::write_str("\",\"ts_ms\":");
    let ms = timer::ticks_to_ms(timer::counter_ticks(), timer::counter_freq_hz());
    uart::write_u64_dec(ms);
    uart::write_str(",\"step\":");
    uart::write_u64_dec(step as u64);
}

fn trace_event(event: &[u8], step: u8) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(event, step);
    uart::write_str("}\n");
}

fn trace_json_escaped(value: &[u8]) {
    let mut i = 0usize;
    while i < value.len() {
        let b = value[i];
        match b {
            b'"' | b'\\' => {
                uart::write_bytes(b"\\");
                uart::write_bytes(&value[i..i + 1]);
            }
            b'\n' => uart::write_bytes(b"\\n"),
            b'\r' => uart::write_bytes(b"\\r"),
            b'\t' => uart::write_bytes(b"\\t"),
            0x00..=0x1f | 0x7f => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                let esc = [
                    b'\\',
                    b'u',
                    b'0',
                    b'0',
                    HEX[((b >> 4) & 0x0f) as usize],
                    HEX[(b & 0x0f) as usize],
                ];
                uart::write_bytes(&esc);
            }
            _ => uart::write_bytes(&value[i..i + 1]),
        }
        i += 1;
    }
}

fn trace_json_string_field(name: &[u8], value: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    uart::write_str(",\"");
    uart::write_bytes(name);
    uart::write_str("\":\"");
    trace_json_escaped(value);
    uart::write_str("\"");
}

fn trace_json_u64_field(name: &[u8], value: u64) {
    if !trace_output_enabled() {
        return;
    }
    uart::write_str(",\"");
    uart::write_bytes(name);
    uart::write_str("\":");
    uart::write_u64_dec(value);
}

fn trace_json_bool_field(name: &[u8], value: bool) {
    if !trace_output_enabled() {
        return;
    }
    uart::write_str(",\"");
    uart::write_bytes(name);
    uart::write_str("\":");
    if value {
        uart::write_str("true");
    } else {
        uart::write_str("false");
    }
}

fn trace_skill_called(skill: &[u8], step: u8) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"skill_called", step);
    uart::write_str(",\"skill\":\"");
    uart::write_bytes(skill);
    uart::write_str("\"}\n");
}

fn trace_skill_result(skill: &[u8], step: u8, status: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"skill_result", step);
    uart::write_str(",\"skill\":\"");
    uart::write_bytes(skill);
    uart::write_str("\",\"status\":\"");
    uart::write_bytes(status);
    uart::write_str("\"}\n");
}

fn trace_policy_checked(skill: &[u8], step: u8, status: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"policy_checked", step);
    uart::write_str(",\"skill\":\"");
    uart::write_bytes(skill);
    uart::write_str("\",\"status\":\"");
    uart::write_bytes(status);
    uart::write_str("\"}\n");
}

fn trace_skill_denied(skill: &[u8], step: u8, reason: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"skill_denied", step);
    uart::write_str(",\"skill\":\"");
    uart::write_bytes(skill);
    uart::write_str("\",\"reason\":\"");
    uart::write_bytes(reason);
    uart::write_str("\"}\n");
}

fn trace_goal_status(event: &[u8], step: u8, status: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(event, step);
    uart::write_str(",\"status\":\"");
    uart::write_bytes(status);
    uart::write_str("\"}\n");
}

fn trace_retry_scheduled(step: u8, phase: &[u8], attempt: u8) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"retry_scheduled", step);
    trace_json_string_field(b"phase", phase);
    trace_json_u64_field(b"attempt", attempt as u64);
    uart::write_str("}\n");
}

pub(crate) fn trace_fetch_phase_changed(phase: &[u8], retry: u8, rounds: u8, proxy: bool) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"fetch_phase_changed", current_trace_step());
    trace_json_string_field(b"phase", phase);
    trace_json_u64_field(b"retry", retry as u64);
    trace_json_u64_field(b"round", rounds as u64);
    trace_json_bool_field(b"proxy", proxy);
    uart::write_str("}\n");
}

pub(crate) fn trace_fetch_cache_hit(cache: &[u8], subject: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"fetch_cache_hit", current_trace_step());
    trace_json_string_field(b"cache", cache);
    trace_json_string_field(b"subject", subject);
    uart::write_str("}\n");
}

fn fetch_failure_reason_or<'a>(default: &'a [u8]) -> &'a [u8] {
    let reason = fetch_error_reason();
    if reason.is_empty() {
        default
    } else {
        reason
    }
}

fn human_status(message: &[u8]) {
    if trace_output_enabled() {
        return;
    }
    if status_inline_enabled() && !debug_output_enabled() {
        show_inline_status(message);
        return;
    }
    clear_inline_status();
    uart::write_bytes(message);
    uart::write_str("\n");
}

fn copy_bytes(dst: &mut [u8], src: &[u8]) -> usize {
    let mut n = src.len();
    if n > dst.len() {
        n = dst.len();
    }
    let mut i = 0usize;
    while i < n {
        dst[i] = src[i];
        i += 1;
    }
    n
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(10 + (b - b'a')),
        b'A'..=b'F' => Some(10 + (b - b'A')),
        _ => None,
    }
}

fn append_utf8_codepoint(out: &mut [u8], n: &mut usize, cp: u32) {
    if cp <= 0x7f {
        if *n < out.len() {
            out[*n] = cp as u8;
            *n += 1;
        }
        return;
    }
    if cp <= 0x7ff {
        if *n + 2 <= out.len() {
            out[*n] = 0b1100_0000 | ((cp >> 6) as u8);
            out[*n + 1] = 0b1000_0000 | ((cp & 0x3f) as u8);
            *n += 2;
        }
        return;
    }
    if cp <= 0xffff {
        if *n + 3 <= out.len() {
            out[*n] = 0b1110_0000 | ((cp >> 12) as u8);
            out[*n + 1] = 0b1000_0000 | (((cp >> 6) & 0x3f) as u8);
            out[*n + 2] = 0b1000_0000 | ((cp & 0x3f) as u8);
            *n += 3;
        }
        return;
    }
    if cp <= 0x10ffff && *n + 4 <= out.len() {
        out[*n] = 0b1111_0000 | ((cp >> 18) as u8);
        out[*n + 1] = 0b1000_0000 | (((cp >> 12) & 0x3f) as u8);
        out[*n + 2] = 0b1000_0000 | (((cp >> 6) & 0x3f) as u8);
        out[*n + 3] = 0b1000_0000 | ((cp & 0x3f) as u8);
        *n += 4;
    }
}

fn is_utf8_continuation(b: u8) -> bool {
    (b & 0b1100_0000) == 0b1000_0000
}

fn utf8_seq_len_at(src: &[u8], idx: usize) -> usize {
    if idx >= src.len() {
        return 0;
    }
    let b0 = src[idx];
    if b0 <= 0x7f {
        return 1;
    }
    if (0xc2..=0xdf).contains(&b0) {
        if idx + 1 < src.len() && is_utf8_continuation(src[idx + 1]) {
            return 2;
        }
        return 0;
    }
    if b0 == 0xe0 {
        if idx + 2 < src.len()
            && (0xa0..=0xbf).contains(&src[idx + 1])
            && is_utf8_continuation(src[idx + 2])
        {
            return 3;
        }
        return 0;
    }
    if (0xe1..=0xec).contains(&b0) || (0xee..=0xef).contains(&b0) {
        if idx + 2 < src.len()
            && is_utf8_continuation(src[idx + 1])
            && is_utf8_continuation(src[idx + 2])
        {
            return 3;
        }
        return 0;
    }
    if b0 == 0xed {
        if idx + 2 < src.len()
            && (0x80..=0x9f).contains(&src[idx + 1])
            && is_utf8_continuation(src[idx + 2])
        {
            return 3;
        }
        return 0;
    }
    if b0 == 0xf0 {
        if idx + 3 < src.len()
            && (0x90..=0xbf).contains(&src[idx + 1])
            && is_utf8_continuation(src[idx + 2])
            && is_utf8_continuation(src[idx + 3])
        {
            return 4;
        }
        return 0;
    }
    if (0xf1..=0xf3).contains(&b0) {
        if idx + 3 < src.len()
            && is_utf8_continuation(src[idx + 1])
            && is_utf8_continuation(src[idx + 2])
            && is_utf8_continuation(src[idx + 3])
        {
            return 4;
        }
        return 0;
    }
    if b0 == 0xf4 {
        if idx + 3 < src.len()
            && (0x80..=0x8f).contains(&src[idx + 1])
            && is_utf8_continuation(src[idx + 2])
            && is_utf8_continuation(src[idx + 3])
        {
            return 4;
        }
        return 0;
    }
    0
}

fn utf8_safe_prefix_len(src: &[u8], max_len: usize) -> usize {
    let limit = if max_len > src.len() { src.len() } else { max_len };
    let mut idx = 0usize;
    let mut last_good = 0usize;
    while idx < limit {
        let seq_len = utf8_seq_len_at(src, idx);
        if seq_len == 0 || idx + seq_len > limit {
            break;
        }
        idx += seq_len;
        last_good = idx;
    }
    last_good
}

fn copy_utf8_prefix(dst: &mut [u8], src: &[u8]) -> usize {
    let n = utf8_safe_prefix_len(src, dst.len());
    copy_bytes(dst, &src[..n])
}

fn json_find_key(buf: &[u8], len: usize, key: &[u8]) -> Option<usize> {
    let mut i = 0usize;
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

fn json_extract_string(buf: &[u8], len: usize, key: &[u8], out: &mut [u8]) -> Option<usize> {
    let mut off = json_find_key(buf, len, key)?;
    while off < len && is_space(buf[off]) {
        off += 1;
    }
    if off >= len || buf[off] != b'"' {
        return None;
    }
    off += 1;
    let mut n = 0usize;
    while off < len {
        let b = buf[off];
        if b == b'"' {
            return Some(n);
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

fn json_extract_u64(buf: &[u8], len: usize, key: &[u8]) -> Option<u64> {
    let mut off = json_find_key(buf, len, key)?;
    while off < len && is_space(buf[off]) {
        off += 1;
    }
    parse_u64(&buf[off..len], len - off)
}

fn json_extract_bool(buf: &[u8], len: usize, key: &[u8]) -> Option<bool> {
    let mut off = json_find_key(buf, len, key)?;
    while off < len && is_space(buf[off]) {
        off += 1;
    }
    if off + 4 <= len && starts_with_at(buf, len, off, b"true") {
        return Some(true);
    }
    if off + 5 <= len && starts_with_at(buf, len, off, b"false") {
        return Some(false);
    }
    None
}

fn json_string_array_contains(buf: &[u8], len: usize, key: &[u8], want: &[u8]) -> Option<bool> {
    let mut off = json_find_key(buf, len, key)?;
    while off < len && is_space(buf[off]) {
        off += 1;
    }
    if off >= len || buf[off] != b'[' {
        return None;
    }
    off += 1;
    loop {
        while off < len && (is_space(buf[off]) || buf[off] == b',') {
            off += 1;
        }
        if off >= len {
            return None;
        }
        if buf[off] == b']' {
            return Some(false);
        }
        if buf[off] != b'"' {
            return None;
        }
        off += 1;
        let mut idx = 0usize;
        let mut matched = true;
        while off < len {
            let mut b = buf[off];
            if b == b'"' {
                break;
            }
            if b == b'\\' {
                off += 1;
                if off >= len {
                    return None;
                }
                b = buf[off];
            }
            if idx >= want.len() || b != want[idx] {
                matched = false;
            }
            idx += 1;
            off += 1;
        }
        if off >= len || buf[off] != b'"' {
            return None;
        }
        if matched && idx == want.len() {
            return Some(true);
        }
        off += 1;
    }
}

fn json_escape_append(out: &mut [u8], mut idx: usize, src: &[u8]) -> usize {
    let mut i = 0usize;
    while i < src.len() && idx < out.len() {
        let b = src[i];
        match b {
            b'"' | b'\\' => {
                if idx + 2 > out.len() {
                    break;
                }
                out[idx] = b'\\';
                out[idx + 1] = b;
                idx += 2;
            }
            b'\n' => {
                if idx + 2 > out.len() {
                    break;
                }
                out[idx] = b'\\';
                out[idx + 1] = b'n';
                idx += 2;
            }
            b'\r' => {
                if idx + 2 > out.len() {
                    break;
                }
                out[idx] = b'\\';
                out[idx + 1] = b'r';
                idx += 2;
            }
            b'\t' => {
                if idx + 2 > out.len() {
                    break;
                }
                out[idx] = b'\\';
                out[idx + 1] = b't';
                idx += 2;
            }
            0x00..=0x1f | 0x7f => {
                if idx + 6 > out.len() {
                    break;
                }
                const HEX: &[u8; 16] = b"0123456789abcdef";
                out[idx] = b'\\';
                out[idx + 1] = b'u';
                out[idx + 2] = b'0';
                out[idx + 3] = b'0';
                out[idx + 4] = HEX[((b >> 4) & 0x0f) as usize];
                out[idx + 5] = HEX[(b & 0x0f) as usize];
                idx += 6;
            }
            _ => {
                let seq_len = utf8_seq_len_at(src, i);
                if seq_len == 0 {
                    append_utf8_codepoint(out, &mut idx, 0xfffd);
                } else {
                    if idx + seq_len > out.len() {
                        break;
                    }
                    let mut j = 0usize;
                    while j < seq_len {
                        out[idx + j] = src[i + j];
                        j += 1;
                    }
                    idx += seq_len;
                    i += seq_len - 1;
                }
            }
        }
        i += 1;
    }
    idx
}

fn agent_reset() {
    r#loop::reset_m4_state();
    unsafe {
        AGENT_MODE = AGENT_MODE_NONE;
        AGENT_TASK_ACTIVE = false;
        AGENT_PHASE = AGENT_PHASE_IDLE;
        AGENT_GOAL_ID_LEN = 0;
        AGENT_GOAL_TEXT_LEN = 0;
        AGENT_SOURCE_URL_LEN = 0;
        AGENT_SINK_URL_LEN = 0;
        AGENT_MODEL_URL_LEN = 0;
        AGENT_TASK_JSON_LEN = 0;
        AGENT_MAX_STEPS = 0;
        AGENT_SUMMARY_LEN = 0;
        AGENT_RESPONSE_BODY_LEN = 0;
        AGENT_OUTPUT_TEXT_LEN = 0;
        AGENT_SUMMARY_SENTENCES = 3;
        AGENT_OUTPUT_LANGUAGE_LEN = 0;
        AGENT_OUTPUT_STYLE_LEN = 0;
        AGENT_RESULT_STATUS_LEN = 0;
        AGENT_RESULT_REASON_LEN = 0;
        AGENT_TERMINAL_KIND = AGENT_TERMINAL_NONE;
        AGENT_OPENAI_INTERPRET_RETRIES = 0;
        AGENT_OPENAI_SUMMARY_RETRIES = 0;
    }
}
