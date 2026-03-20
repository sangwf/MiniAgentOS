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

static mut MODEL_TRACE_NEXT_INTERACTION_ID: u32 = 0;
static mut MODEL_TRACE_CURRENT_INTERACTION_ID: u32 = 0;

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

fn trace_json_hex_field(name: &[u8], value: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    if !trace_output_enabled() {
        return;
    }
    uart::write_str(",\"");
    uart::write_bytes(name);
    uart::write_str("\":\"");
    let mut i = 0usize;
    while i < value.len() {
        let pair = [
            HEX[((value[i] >> 4) & 0x0f) as usize],
            HEX[(value[i] & 0x0f) as usize],
        ];
        uart::write_bytes(&pair);
        i += 1;
    }
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

fn trace_json_i64_field(name: &[u8], value: i64) {
    if !trace_output_enabled() {
        return;
    }
    uart::write_str(",\"");
    uart::write_bytes(name);
    uart::write_str("\":");
    if value < 0 {
        uart::write_str("-");
        uart::write_u64_dec(value.wrapping_neg() as u64);
    } else {
        uart::write_u64_dec(value as u64);
    }
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

fn trace_next_model_interaction_id() -> u32 {
    unsafe {
        MODEL_TRACE_NEXT_INTERACTION_ID = MODEL_TRACE_NEXT_INTERACTION_ID.wrapping_add(1);
        if MODEL_TRACE_NEXT_INTERACTION_ID == 0 {
            MODEL_TRACE_NEXT_INTERACTION_ID = 1;
        }
        MODEL_TRACE_CURRENT_INTERACTION_ID = MODEL_TRACE_NEXT_INTERACTION_ID;
        MODEL_TRACE_CURRENT_INTERACTION_ID
    }
}

fn trace_current_model_interaction_id() -> u32 {
    unsafe { MODEL_TRACE_CURRENT_INTERACTION_ID }
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

pub(crate) fn trace_tls_handshake_failure(
    ret: i32,
    x509_err: i32,
    curve_id: i32,
    skx_err: i32,
    skx_ret: i32,
    verify_flags: u32,
    state: i32,
    state_label: &[u8],
    label: &[u8],
) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"tls_handshake_failure", current_trace_step());
    trace_json_i64_field(b"ret", ret as i64);
    trace_json_i64_field(b"x509_err", x509_err as i64);
    trace_json_i64_field(b"curve_id", curve_id as i64);
    trace_json_i64_field(b"skx_err", skx_err as i64);
    trace_json_i64_field(b"skx_ret", skx_ret as i64);
    trace_json_u64_field(b"verify_flags", verify_flags as u64);
    trace_json_i64_field(b"state", state as i64);
    trace_json_string_field(b"state_label", state_label);
    trace_json_string_field(b"label", label);
    uart::write_str("}\n");
}

pub(crate) fn trace_tls_io_failure(
    phase: &[u8],
    ret: i32,
    verify_flags: u32,
    pending: bool,
    state: i32,
    state_label: &[u8],
    label: &[u8],
) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"tls_io_failure", current_trace_step());
    trace_json_string_field(b"phase", phase);
    trace_json_i64_field(b"ret", ret as i64);
    trace_json_u64_field(b"verify_flags", verify_flags as u64);
    trace_json_bool_field(b"pending", pending);
    trace_json_i64_field(b"state", state as i64);
    trace_json_string_field(b"state_label", state_label);
    trace_json_string_field(b"label", label);
    uart::write_str("}\n");
}

pub(crate) fn trace_tls_config(
    reset_ret: i32,
    hostname_ret: i32,
    state: i32,
    state_label: &[u8],
    in_ctr: u64,
    out_ctr: u64,
    has_transform_out: bool,
    aes256_zero_key_self_hash: u64,
) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"tls_config", current_trace_step());
    trace_json_i64_field(b"reset_ret", reset_ret as i64);
    trace_json_i64_field(b"hostname_ret", hostname_ret as i64);
    trace_json_i64_field(b"state", state as i64);
    trace_json_string_field(b"state_label", state_label);
    trace_json_u64_field(b"in_ctr", in_ctr);
    trace_json_u64_field(b"out_ctr", out_ctr);
    trace_json_bool_field(b"has_transform_out", has_transform_out);
    trace_json_u64_field(b"aes256_zero_key_self_hash", aes256_zero_key_self_hash);
    uart::write_str("}\n");
}

pub(crate) fn trace_tls_close_notify(ret: i32, verify_flags: u32, pending: bool, label: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"tls_close_notify", current_trace_step());
    trace_json_i64_field(b"ret", ret as i64);
    trace_json_u64_field(b"verify_flags", verify_flags as u64);
    trace_json_bool_field(b"pending", pending);
    trace_json_string_field(b"label", label);
    uart::write_str("}\n");
}

pub(crate) fn trace_tls_export(
    count: u32,
    client_random_prefix: u64,
    server_random_prefix: u64,
    client_random_hash: u64,
    server_random_hash: u64,
    master_hash: u64,
    keyblock_hash: u64,
    client_write_mac_hash: u64,
    client_write_key_hash: u64,
    client_write_key_prefix: u64,
    server_write_key_hash: u64,
    client_write_key_aes_zero_hash: u64,
    client_write_key_aes_zero_hash_static: u64,
    aes256_zero_key_self_hash: u64,
    maclen: u32,
    keylen: u32,
    ivlen: u32,
    prf_type: i32,
    in_ctr: u64,
    out_ctr: u64,
    has_transform_out: bool,
) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"tls_export", current_trace_step());
    trace_json_u64_field(b"count", count as u64);
    trace_json_u64_field(b"client_random_prefix", client_random_prefix);
    trace_json_u64_field(b"server_random_prefix", server_random_prefix);
    trace_json_u64_field(b"client_random_hash", client_random_hash);
    trace_json_u64_field(b"server_random_hash", server_random_hash);
    trace_json_u64_field(b"master_hash", master_hash);
    trace_json_u64_field(b"keyblock_hash", keyblock_hash);
    trace_json_u64_field(b"client_write_mac_hash", client_write_mac_hash);
    trace_json_u64_field(b"client_write_key_hash", client_write_key_hash);
    trace_json_u64_field(b"client_write_key_prefix", client_write_key_prefix);
    trace_json_u64_field(b"server_write_key_hash", server_write_key_hash);
    trace_json_u64_field(
        b"client_write_key_aes_zero_hash",
        client_write_key_aes_zero_hash,
    );
    trace_json_u64_field(
        b"client_write_key_aes_zero_hash_static",
        client_write_key_aes_zero_hash_static,
    );
    trace_json_u64_field(b"aes256_zero_key_self_hash", aes256_zero_key_self_hash);
    trace_json_u64_field(b"maclen", maclen as u64);
    trace_json_u64_field(b"keylen", keylen as u64);
    trace_json_u64_field(b"ivlen", ivlen as u64);
    trace_json_i64_field(b"prf_type", prf_type as i64);
    trace_json_u64_field(b"in_ctr", in_ctr);
    trace_json_u64_field(b"out_ctr", out_ctr);
    trace_json_bool_field(b"has_transform_out", has_transform_out);
    uart::write_str("}\n");
}

pub(crate) fn trace_tls_last_tx(state: i32, state_label: &[u8], out_ctr: u64, record: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"tls_last_tx", current_trace_step());
    trace_json_i64_field(b"state", state as i64);
    trace_json_string_field(b"state_label", state_label);
    trace_json_u64_field(b"out_ctr", out_ctr);
    trace_json_u64_field(b"len", record.len() as u64);
    trace_json_hex_field(b"record_hex", record);
    uart::write_str("}\n");
}

pub(crate) fn trace_tls_cipher_diag(
    ciphersuite: i32,
    cipher_type: i32,
    cipher_mode: i32,
    cipher_operation: i32,
    cipher_key_bitlen: u32,
    iv_enc_prefix: u64,
    iv_enc_hash: u64,
    cipher_ctx_enc_aes_zero_hash: u64,
) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"tls_cipher_diag", current_trace_step());
    trace_json_i64_field(b"ciphersuite", ciphersuite as i64);
    trace_json_i64_field(b"cipher_type", cipher_type as i64);
    trace_json_i64_field(b"cipher_mode", cipher_mode as i64);
    trace_json_i64_field(b"cipher_operation", cipher_operation as i64);
    trace_json_u64_field(b"cipher_key_bitlen", cipher_key_bitlen as u64);
    trace_json_u64_field(b"iv_enc_prefix", iv_enc_prefix);
    trace_json_u64_field(b"iv_enc_hash", iv_enc_hash);
    trace_json_u64_field(
        b"cipher_ctx_enc_aes_zero_hash",
        cipher_ctx_enc_aes_zero_hash,
    );
    uart::write_str("}\n");
}

pub(crate) fn trace_tls_record_diag(
    decrypt_ok: bool,
    plaintext_hash: u64,
    plaintext_len: u32,
    padlen: u32,
) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"tls_record_diag", current_trace_step());
    trace_json_bool_field(b"decrypt_ok", decrypt_ok);
    trace_json_u64_field(b"plaintext_hash", plaintext_hash);
    trace_json_u64_field(b"plaintext_len", plaintext_len as u64);
    trace_json_u64_field(b"padlen", padlen as u64);
    uart::write_str("}\n");
}

pub(crate) fn trace_tls_cbc_diag(
    reencrypt_match: bool,
    plain_hash: u64,
    expected_cipher_hash: u64,
    actual_cipher_hash: u64,
    cipher_len: u32,
) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"tls_cbc_diag", current_trace_step());
    trace_json_bool_field(b"reencrypt_match", reencrypt_match);
    trace_json_u64_field(b"plain_hash", plain_hash);
    trace_json_u64_field(b"expected_cipher_hash", expected_cipher_hash);
    trace_json_u64_field(b"actual_cipher_hash", actual_cipher_hash);
    trace_json_u64_field(b"cipher_len", cipher_len as u64);
    uart::write_str("}\n");
}

pub(crate) fn trace_tls_mac_diag(mac_match: bool, expected_mac_hash: u64, actual_mac_hash: u64) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"tls_mac_diag", current_trace_step());
    trace_json_bool_field(b"mac_match", mac_match);
    trace_json_u64_field(b"expected_mac_hash", expected_mac_hash);
    trace_json_u64_field(b"actual_mac_hash", actual_mac_hash);
    uart::write_str("}\n");
}

pub(crate) fn trace_model_output_preview(text: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"model_output_preview", current_trace_step());
    trace_json_string_field(b"text", text);
    uart::write_str("}\n");
}

pub(crate) fn trace_model_request_snapshot(
    phase: &[u8],
    model: &[u8],
    instructions: &[u8],
    input: &[u8],
    reasoning_effort: &[u8],
    max_output_tokens: u64,
) {
    if !trace_output_enabled() {
        return;
    }
    let interaction_id = trace_next_model_interaction_id();
    trace_begin(b"model_request_snapshot", current_trace_step());
    trace_json_u64_field(b"interaction_id", interaction_id as u64);
    trace_json_string_field(b"phase", phase);
    trace_json_string_field(b"model", model);
    trace_json_string_field(b"instructions", instructions);
    trace_json_string_field(b"input", input);
    trace_json_string_field(b"reasoning_effort", reasoning_effort);
    trace_json_u64_field(b"max_output_tokens", max_output_tokens);
    uart::write_str("}\n");
}

pub(crate) fn trace_model_response_snapshot(
    phase: &[u8],
    http_status: u16,
    body_truncated: bool,
    parsed_output: bool,
    text: &[u8],
) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"model_response_snapshot", current_trace_step());
    trace_json_u64_field(
        b"interaction_id",
        trace_current_model_interaction_id() as u64,
    );
    trace_json_string_field(b"phase", phase);
    trace_json_u64_field(b"http_status", http_status as u64);
    trace_json_bool_field(b"body_truncated", body_truncated);
    trace_json_bool_field(b"parsed_output", parsed_output);
    trace_json_string_field(b"text", text);
    uart::write_str("}\n");
}

pub(crate) fn trace_model_parse_error(reason: &[u8]) {
    if !trace_output_enabled() {
        return;
    }
    trace_begin(b"model_parse_error", current_trace_step());
    trace_json_string_field(b"reason", reason);
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
