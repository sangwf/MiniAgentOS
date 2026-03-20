use super::*;

const M7_MEMORY_SLOT_COUNT: usize = 5;
const M7_MEMORY_ID_MAX: usize = 24;
const M7_MEMORY_SUMMARY_MAX: usize = 192;
const M7_MEMORY_DETAIL_MAX: usize = 512;
const M7_MEMORY_SOURCE_MAX: usize = 128;
const M7_COMPACTION_TRIGGER_CHARS: usize = 240;
const M7_COMPACTION_EXCERPT_MAX: usize = 192;

const M7_KIND_NONE: u8 = 0;
const M7_KIND_TASK: u8 = 1;
const M7_KIND_SOURCE: u8 = 2;
const M7_KIND_WORKSPACE: u8 = 3;
const M7_KIND_EXECUTION: u8 = 4;
const M7_KIND_CONVERSATION: u8 = 5;

const M7_STATE_NONE: u8 = 0;
const M7_STATE_RAW: u8 = 1;
const M7_STATE_COMPACTED: u8 = 2;
const M7_STATE_DERIVED: u8 = 3;

const M7_SLOT_TASK: usize = 0;
const M7_SLOT_SOURCE: usize = 1;
const M7_SLOT_WORKSPACE: usize = 2;
const M7_SLOT_EXECUTION: usize = 3;
const M7_SLOT_CONVERSATION: usize = 4;

static mut M7_MEMORY_STARTED: bool = false;
static mut M7_MEMORY_TURN: u16 = 0;
static mut M7_MEMORY_KIND: [u8; M7_MEMORY_SLOT_COUNT] = [M7_KIND_NONE; M7_MEMORY_SLOT_COUNT];
static mut M7_MEMORY_STATE: [u8; M7_MEMORY_SLOT_COUNT] = [M7_STATE_NONE; M7_MEMORY_SLOT_COUNT];
static mut M7_MEMORY_ID: [[u8; M7_MEMORY_ID_MAX]; M7_MEMORY_SLOT_COUNT] =
    [[0u8; M7_MEMORY_ID_MAX]; M7_MEMORY_SLOT_COUNT];
static mut M7_MEMORY_ID_LEN: [usize; M7_MEMORY_SLOT_COUNT] = [0usize; M7_MEMORY_SLOT_COUNT];
static mut M7_MEMORY_SUMMARY: [[u8; M7_MEMORY_SUMMARY_MAX]; M7_MEMORY_SLOT_COUNT] =
    [[0u8; M7_MEMORY_SUMMARY_MAX]; M7_MEMORY_SLOT_COUNT];
static mut M7_MEMORY_SUMMARY_LEN: [usize; M7_MEMORY_SLOT_COUNT] = [0usize; M7_MEMORY_SLOT_COUNT];
static mut M7_MEMORY_DETAIL: [[u8; M7_MEMORY_DETAIL_MAX]; M7_MEMORY_SLOT_COUNT] =
    [[0u8; M7_MEMORY_DETAIL_MAX]; M7_MEMORY_SLOT_COUNT];
static mut M7_MEMORY_DETAIL_LEN: [usize; M7_MEMORY_SLOT_COUNT] = [0usize; M7_MEMORY_SLOT_COUNT];
static mut M7_MEMORY_SOURCE: [[u8; M7_MEMORY_SOURCE_MAX]; M7_MEMORY_SLOT_COUNT] =
    [[0u8; M7_MEMORY_SOURCE_MAX]; M7_MEMORY_SLOT_COUNT];
static mut M7_MEMORY_SOURCE_LEN: [usize; M7_MEMORY_SLOT_COUNT] = [0usize; M7_MEMORY_SLOT_COUNT];
static mut M7_MEMORY_CREATED_TURN: [u16; M7_MEMORY_SLOT_COUNT] = [0u16; M7_MEMORY_SLOT_COUNT];
static mut M7_MEMORY_UPDATED_TURN: [u16; M7_MEMORY_SLOT_COUNT] = [0u16; M7_MEMORY_SLOT_COUNT];

static mut M7_BUDGET_INSTRUCTIONS_CHARS: usize = 0;
static mut M7_BUDGET_CURRENT_REQUEST_CHARS: usize = 0;
static mut M7_BUDGET_LATEST_TOOL_RESULT_CHARS: usize = 0;
static mut M7_BUDGET_WORKING_MEMORY_CHARS: usize = 0;
static mut M7_BUDGET_KNOWN_SOURCES_CHARS: usize = 0;
static mut M7_BUDGET_WORKSPACE_MEMORY_CHARS: usize = 0;
static mut M7_BUDGET_SESSION_STATE_CHARS: usize = 0;
static mut M7_BUDGET_RECENT_CONVERSATION_CHARS: usize = 0;
static mut M7_BUDGET_ESTIMATED_TOTAL_TOKENS: usize = 0;

fn estimate_tokens(chars: usize) -> usize {
    if chars == 0 {
        0
    } else {
        (chars + 3) / 4
    }
}

fn u64_to_dec_local(out: &mut [u8], mut value: u64) -> usize {
    if out.is_empty() {
        return 0;
    }
    if value == 0 {
        out[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 20];
    let mut len = 0usize;
    while value != 0 && len < tmp.len() {
        tmp[len] = b'0' + (value % 10) as u8;
        value /= 10;
        len += 1;
    }
    let mut i = 0usize;
    while i < len && i < out.len() {
        out[i] = tmp[len - 1 - i];
        i += 1;
    }
    i
}

fn bytes_eq_local(a: &[u8], b: &[u8]) -> bool {
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

fn slot_id_bytes(slot: usize) -> &'static [u8] {
    match slot {
        M7_SLOT_TASK => b"mem-task",
        M7_SLOT_SOURCE => b"mem-source",
        M7_SLOT_WORKSPACE => b"mem-workspace",
        M7_SLOT_EXECUTION => b"mem-execution",
        M7_SLOT_CONVERSATION => b"mem-conversation",
        _ => b"mem-unknown",
    }
}

fn kind_name(kind: u8) -> &'static [u8] {
    match kind {
        M7_KIND_TASK => b"task",
        M7_KIND_SOURCE => b"source",
        M7_KIND_WORKSPACE => b"workspace",
        M7_KIND_EXECUTION => b"execution",
        M7_KIND_CONVERSATION => b"conversation",
        _ => b"unknown",
    }
}

fn state_name(state: u8) -> &'static [u8] {
    match state {
        M7_STATE_RAW => b"raw",
        M7_STATE_COMPACTED => b"compacted",
        M7_STATE_DERIVED => b"derived",
        _ => b"unknown",
    }
}

fn ensure_started() {
    if unsafe { M7_MEMORY_STARTED } {
        return;
    }
    memory_reset();
}

fn clear_slot(slot: usize) {
    unsafe {
        M7_MEMORY_KIND[slot] = M7_KIND_NONE;
        M7_MEMORY_STATE[slot] = M7_STATE_NONE;
        M7_MEMORY_ID_LEN[slot] = 0;
        M7_MEMORY_SUMMARY_LEN[slot] = 0;
        M7_MEMORY_DETAIL_LEN[slot] = 0;
        M7_MEMORY_SOURCE_LEN[slot] = 0;
        M7_MEMORY_CREATED_TURN[slot] = 0;
        M7_MEMORY_UPDATED_TURN[slot] = 0;
    }
}

fn trace_memory_event_local(event: &[u8], slot: usize, from_state: u8, to_state: u8) {
    if !trace_output_enabled() {
        return;
    }
    unsafe {
        trace_begin(event, super::current_trace_step());
        trace_json_string_field(b"entry_id", &M7_MEMORY_ID[slot][..M7_MEMORY_ID_LEN[slot]]);
        trace_json_string_field(b"kind", kind_name(M7_MEMORY_KIND[slot]));
        if from_state != M7_STATE_NONE {
            trace_json_string_field(b"from_state", state_name(from_state));
        }
        trace_json_string_field(b"to_state", state_name(to_state));
        trace_json_u64_field(b"turn_index", M7_MEMORY_UPDATED_TURN[slot] as u64);
        uart::write_str("}\n");
    }
}

fn trace_memory_entry_snapshot_local(slot: usize) {
    if !trace_output_enabled() {
        return;
    }
    unsafe {
        if M7_MEMORY_KIND[slot] == M7_KIND_NONE || M7_MEMORY_SUMMARY_LEN[slot] == 0 {
            return;
        }
        trace_begin(b"memory_entry_snapshot", super::current_trace_step());
        trace_json_string_field(b"id", &M7_MEMORY_ID[slot][..M7_MEMORY_ID_LEN[slot]]);
        trace_json_string_field(b"kind", kind_name(M7_MEMORY_KIND[slot]));
        trace_json_string_field(b"summary", &M7_MEMORY_SUMMARY[slot][..M7_MEMORY_SUMMARY_LEN[slot]]);
        trace_json_string_field(b"source", &M7_MEMORY_SOURCE[slot][..M7_MEMORY_SOURCE_LEN[slot]]);
        trace_json_string_field(b"state", state_name(M7_MEMORY_STATE[slot]));
        trace_json_u64_field(b"created_turn", M7_MEMORY_CREATED_TURN[slot] as u64);
        trace_json_u64_field(b"updated_turn", M7_MEMORY_UPDATED_TURN[slot] as u64);
        trace_json_u64_field(
            b"chars",
            (M7_MEMORY_SUMMARY_LEN[slot] + M7_MEMORY_DETAIL_LEN[slot]) as u64,
        );
        trace_json_u64_field(
            b"estimated_tokens",
            estimate_tokens(M7_MEMORY_SUMMARY_LEN[slot] + M7_MEMORY_DETAIL_LEN[slot]) as u64,
        );
        uart::write_str("}\n");
    }
}

fn trace_memory_compacted_local(
    slot: usize,
    from_state: u8,
    source_chars: usize,
    retained_chars: usize,
    mode: &[u8],
) {
    if !trace_output_enabled() {
        return;
    }
    unsafe {
        trace_begin(b"memory_compacted", super::current_trace_step());
        trace_json_string_field(b"entry_id", &M7_MEMORY_ID[slot][..M7_MEMORY_ID_LEN[slot]]);
        trace_json_string_field(b"kind", kind_name(M7_MEMORY_KIND[slot]));
        if from_state != M7_STATE_NONE {
            trace_json_string_field(b"from_state", state_name(from_state));
        }
        trace_json_string_field(b"to_state", b"compacted");
        trace_json_string_field(b"mode", mode);
        trace_json_u64_field(b"source_chars", source_chars as u64);
        trace_json_u64_field(b"retained_chars", retained_chars as u64);
        trace_json_u64_field(
            b"dropped_chars",
            source_chars.saturating_sub(retained_chars) as u64,
        );
        trace_json_u64_field(b"turn_index", M7_MEMORY_UPDATED_TURN[slot] as u64);
        uart::write_str("}\n");
    }
}

fn set_slot(
    slot: usize,
    kind: u8,
    state: u8,
    source: &[u8],
    summary: &[u8],
    detail: &[u8],
) -> u8 {
    ensure_started();
    let previous_state = unsafe { M7_MEMORY_STATE[slot] };
    unsafe {
        if M7_MEMORY_ID_LEN[slot] == 0 {
            M7_MEMORY_ID_LEN[slot] = copy_bytes(&mut M7_MEMORY_ID[slot], slot_id_bytes(slot));
            M7_MEMORY_CREATED_TURN[slot] = M7_MEMORY_TURN;
        }
        M7_MEMORY_KIND[slot] = kind;
        M7_MEMORY_STATE[slot] = state;
        M7_MEMORY_SOURCE_LEN[slot] = copy_utf8_prefix(&mut M7_MEMORY_SOURCE[slot], source);
        M7_MEMORY_SUMMARY_LEN[slot] = copy_utf8_prefix(&mut M7_MEMORY_SUMMARY[slot], summary);
        M7_MEMORY_DETAIL_LEN[slot] = copy_utf8_prefix(&mut M7_MEMORY_DETAIL[slot], detail);
        M7_MEMORY_UPDATED_TURN[slot] = M7_MEMORY_TURN;
    }
    trace_memory_event_local(b"memory_event", slot, previous_state, state);
    trace_memory_entry_snapshot_local(slot);
    previous_state
}

fn count_occurrences_local(buf: &[u8], pat: &[u8]) -> usize {
    if pat.is_empty() || buf.len() < pat.len() {
        return 0;
    }
    let mut count = 0usize;
    let mut i = 0usize;
    while i + pat.len() <= buf.len() {
        if &buf[i..i + pat.len()] == pat {
            count += 1;
            i += pat.len();
        } else {
            i += 1;
        }
    }
    count
}

fn contains_bytes_local(buf: &[u8], pat: &[u8]) -> bool {
    count_occurrences_local(buf, pat) != 0
}

fn trimmed_len_local(buf: &[u8], mut len: usize) -> usize {
    while len != 0 {
        let ch = buf[len - 1];
        if ch == b' ' || ch == b'\n' || ch == b'\r' || ch == b'\t' {
            len -= 1;
        } else {
            break;
        }
    }
    len
}

fn normalized_excerpt_from(out: &mut [u8], src: &[u8]) -> usize {
    let mut raw = [0u8; M7_COMPACTION_EXCERPT_MAX];
    let raw_len = copy_utf8_prefix(&mut raw, src);
    let mut idx = 0usize;
    let mut last_space = true;
    let mut i = 0usize;
    while i < raw_len && idx < out.len() {
        let ch = raw[i];
        if ch == b'\n' || ch == b'\r' || ch == b'\t' || ch == b' ' {
            if !last_space && idx < out.len() {
                out[idx] = b' ';
                idx += 1;
                last_space = true;
            }
        } else {
            out[idx] = ch;
            idx += 1;
            last_space = false;
        }
        i += 1;
    }
    idx = trimmed_len_local(out, idx);
    if src.len() > raw_len && idx + 3 <= out.len() {
        out[idx] = b'.';
        out[idx + 1] = b'.';
        out[idx + 2] = b'.';
        idx += 3;
    }
    idx
}

fn append_normalized_excerpt(out: &mut [u8], idx: &mut usize, src: &[u8]) {
    let mut excerpt = [0u8; M7_COMPACTION_EXCERPT_MAX];
    let excerpt_len = normalized_excerpt_from(&mut excerpt, src);
    *idx += copy_bytes(&mut out[*idx..], &excerpt[..excerpt_len]);
}

fn append_u64_jsonless(out: &mut [u8], idx: &mut usize, value: u64) {
    *idx += u64_to_dec_local(&mut out[*idx..], value);
}

fn build_generic_compacted_detail(
    out: &mut [u8],
    tool: &[u8],
    result: &[u8],
    source_label: &[u8],
) -> usize {
    let mut idx = 0usize;
    idx += copy_bytes(&mut out[idx..], b"Retained ");
    idx += copy_bytes(&mut out[idx..], source_label);
    idx += copy_bytes(&mut out[idx..], b" excerpt from ");
    idx += copy_bytes(&mut out[idx..], tool);
    idx += copy_bytes(&mut out[idx..], b": ");
    append_normalized_excerpt(out, &mut idx, result);
    idx += copy_bytes(&mut out[idx..], b". Full raw result not carried forward.");
    idx
}

fn build_search_compacted(summary_out: &mut [u8], detail_out: &mut [u8], result: &[u8]) -> (usize, usize) {
    let result_count = count_occurrences_local(result, b"\"url\":\"");
    let mut title = [0u8; 160];
    let title_len = json_extract_string(result, result.len(), b"title", &mut title).unwrap_or(0);
    let mut url = [0u8; 160];
    let url_len = json_extract_string(result, result.len(), b"url", &mut url).unwrap_or(0);
    let mut snippet = [0u8; 256];
    let snippet_len = json_extract_string(result, result.len(), b"snippet", &mut snippet).unwrap_or(0);

    let mut summary_len = 0usize;
    summary_len += copy_bytes(&mut summary_out[summary_len..], b"Compacted search results from search_web");
    if result_count != 0 {
        summary_len += copy_bytes(&mut summary_out[summary_len..], b" (");
        append_u64_jsonless(summary_out, &mut summary_len, result_count as u64);
        summary_len += copy_bytes(&mut summary_out[summary_len..], b" results)");
    }
    summary_len += copy_bytes(&mut summary_out[summary_len..], b".");

    let mut detail_len = 0usize;
    detail_len += copy_bytes(&mut detail_out[detail_len..], b"Retained top result");
    if title_len != 0 {
        detail_len += copy_bytes(&mut detail_out[detail_len..], b": ");
        append_normalized_excerpt(detail_out, &mut detail_len, &title[..title_len]);
    }
    if url_len != 0 {
        detail_len += copy_bytes(&mut detail_out[detail_len..], b" | ");
        append_normalized_excerpt(detail_out, &mut detail_len, &url[..url_len]);
    }
    if snippet_len != 0 {
        detail_len += copy_bytes(&mut detail_out[detail_len..], b" | ");
        append_normalized_excerpt(detail_out, &mut detail_len, &snippet[..snippet_len]);
    }
    detail_len += copy_bytes(&mut detail_out[detail_len..], b". Full raw search JSON not carried forward.");
    (summary_len, detail_len)
}

fn build_fetch_compacted(summary_out: &mut [u8], detail_out: &mut [u8], result: &[u8]) -> (usize, usize) {
    let prefix = b"Fetched URL content preview:\n";
    let body = if starts_with(result, result.len(), prefix) {
        &result[prefix.len()..]
    } else {
        result
    };
    let mut summary_len = 0usize;
    summary_len += copy_bytes(&mut summary_out[summary_len..], b"Compacted fetched source from fetch_url.");
    let mut detail_len = 0usize;
    detail_len += copy_bytes(&mut detail_out[detail_len..], b"Retained excerpt: ");
    append_normalized_excerpt(detail_out, &mut detail_len, body);
    detail_len += copy_bytes(&mut detail_out[detail_len..], b". Full fetched page content not carried forward.");
    if contains_bytes_local(result, b"...(truncated)") {
        detail_len += copy_bytes(
            &mut detail_out[detail_len..],
            b" The original preview was already truncated before retention.",
        );
    }
    (summary_len, detail_len)
}

fn build_execution_compacted(summary_out: &mut [u8], detail_out: &mut [u8], result: &[u8]) -> (usize, usize) {
    let exit_code = json_extract_u64(result, result.len(), b"exit_code").unwrap_or(0);
    let mut stdout = [0u8; 256];
    let stdout_len = json_extract_string(result, result.len(), b"stdout", &mut stdout).unwrap_or(0);
    let mut stderr = [0u8; 256];
    let stderr_len = json_extract_string(result, result.len(), b"stderr", &mut stderr).unwrap_or(0);

    let mut summary_len = 0usize;
    if exit_code == 0 {
        summary_len += copy_bytes(
            &mut summary_out[summary_len..],
            b"Compacted successful execution result from read_process_output (exit_code=",
        );
    } else {
        summary_len += copy_bytes(
            &mut summary_out[summary_len..],
            b"Compacted failed execution result from read_process_output (exit_code=",
        );
    }
    append_u64_jsonless(summary_out, &mut summary_len, exit_code);
    summary_len += copy_bytes(&mut summary_out[summary_len..], b").");

    let mut detail_len = 0usize;
    detail_len += copy_bytes(&mut detail_out[detail_len..], b"Retained exit_code=");
    append_u64_jsonless(detail_out, &mut detail_len, exit_code);
    if stdout_len != 0 {
        detail_len += copy_bytes(&mut detail_out[detail_len..], b"; stdout: ");
        append_normalized_excerpt(detail_out, &mut detail_len, &stdout[..stdout_len]);
    }
    if stderr_len != 0 {
        detail_len += copy_bytes(&mut detail_out[detail_len..], b"; stderr: ");
        append_normalized_excerpt(detail_out, &mut detail_len, &stderr[..stderr_len]);
    }
    detail_len += copy_bytes(&mut detail_out[detail_len..], b". Full raw process output not carried forward.");
    (summary_len, detail_len)
}

fn build_workspace_compacted(summary_out: &mut [u8], detail_out: &mut [u8], tool: &[u8], result: &[u8]) -> (usize, usize) {
    let mut summary_len = 0usize;
    summary_len += copy_bytes(&mut summary_out[summary_len..], b"Compacted workspace result from ");
    summary_len += copy_bytes(&mut summary_out[summary_len..], tool);
    summary_len += copy_bytes(&mut summary_out[summary_len..], b".");
    let detail_len = build_generic_compacted_detail(detail_out, tool, result, b"workspace");
    (summary_len, detail_len)
}

fn build_conversation_compacted(summary_out: &mut [u8], detail_out: &mut [u8], text: &[u8]) -> (usize, usize) {
    let mut summary_len = 0usize;
    summary_len += copy_bytes(&mut summary_out[summary_len..], b"Compacted assistant response.");
    let mut detail_len = 0usize;
    detail_len += copy_bytes(&mut detail_out[detail_len..], b"Retained response excerpt: ");
    append_normalized_excerpt(detail_out, &mut detail_len, text);
    detail_len += copy_bytes(&mut detail_out[detail_len..], b". Full response text not carried forward.");
    (summary_len, detail_len)
}

fn compact_tool_result_if_needed(
    tool: &[u8],
    result: &[u8],
    summary_out: &mut [u8],
    detail_out: &mut [u8],
) -> Option<(usize, usize)> {
    let should_compact = result.len() > M7_COMPACTION_TRIGGER_CHARS
        || (starts_with(tool, tool.len(), b"fetch_url") && result.len() > 160)
        || (starts_with(tool, tool.len(), b"search_web") && result.len() > 160)
        || (starts_with(tool, tool.len(), b"read_process_output") && result.len() > 160)
        || (starts_with(tool, tool.len(), b"read_file") && result.len() > 160);
    if !should_compact {
        return None;
    }
    if starts_with(tool, tool.len(), b"search_web") {
        return Some(build_search_compacted(summary_out, detail_out, result));
    }
    if starts_with(tool, tool.len(), b"fetch_url") {
        return Some(build_fetch_compacted(summary_out, detail_out, result));
    }
    if starts_with(tool, tool.len(), b"read_process_output") {
        return Some(build_execution_compacted(summary_out, detail_out, result));
    }
    if starts_with(tool, tool.len(), b"list_workspace")
        || starts_with(tool, tool.len(), b"read_file")
        || starts_with(tool, tool.len(), b"write_file")
        || starts_with(tool, tool.len(), b"apply_patch")
    {
        return Some(build_workspace_compacted(summary_out, detail_out, tool, result));
    }
    None
}

fn append_entry_json(out: &mut [u8], idx: &mut usize, slot: usize, include_detail: bool) {
    unsafe {
        *idx += copy_bytes(&mut out[*idx..], b"{\"id\":\"");
        *idx = json_escape_append(out, *idx, &M7_MEMORY_ID[slot][..M7_MEMORY_ID_LEN[slot]]);
        *idx += copy_bytes(&mut out[*idx..], b"\",\"kind\":\"");
        *idx += copy_bytes(&mut out[*idx..], kind_name(M7_MEMORY_KIND[slot]));
        *idx += copy_bytes(&mut out[*idx..], b"\",\"summary\":\"");
        *idx = json_escape_append(out, *idx, &M7_MEMORY_SUMMARY[slot][..M7_MEMORY_SUMMARY_LEN[slot]]);
        if include_detail {
            *idx += copy_bytes(&mut out[*idx..], b"\",\"detail\":\"");
            *idx = json_escape_append(out, *idx, &M7_MEMORY_DETAIL[slot][..M7_MEMORY_DETAIL_LEN[slot]]);
        }
        *idx += copy_bytes(&mut out[*idx..], b"\",\"source\":\"");
        *idx = json_escape_append(out, *idx, &M7_MEMORY_SOURCE[slot][..M7_MEMORY_SOURCE_LEN[slot]]);
        *idx += copy_bytes(&mut out[*idx..], b"\",\"state\":\"");
        *idx += copy_bytes(&mut out[*idx..], state_name(M7_MEMORY_STATE[slot]));
        *idx += copy_bytes(&mut out[*idx..], b"\",\"created_turn\":");
        *idx += u64_to_dec_local(&mut out[*idx..], M7_MEMORY_CREATED_TURN[slot] as u64);
        *idx += copy_bytes(&mut out[*idx..], b",\"updated_turn\":");
        *idx += u64_to_dec_local(&mut out[*idx..], M7_MEMORY_UPDATED_TURN[slot] as u64);
        let chars = M7_MEMORY_SUMMARY_LEN[slot] + M7_MEMORY_DETAIL_LEN[slot];
        *idx += copy_bytes(&mut out[*idx..], b",\"chars\":");
        *idx += u64_to_dec_local(&mut out[*idx..], chars as u64);
        *idx += copy_bytes(&mut out[*idx..], b",\"estimated_tokens\":");
        *idx += u64_to_dec_local(&mut out[*idx..], estimate_tokens(chars) as u64);
        *idx += copy_bytes(&mut out[*idx..], b"}");
    }
}

fn append_entry_prompt_line(out: &mut [u8], idx: &mut usize, slot: usize, remaining: usize) -> usize {
    let mut line = [0u8; 896];
    let mut line_len = 0usize;
    unsafe {
        line_len += copy_bytes(&mut line[line_len..], b"- [");
        line_len += copy_bytes(&mut line[line_len..], kind_name(M7_MEMORY_KIND[slot]));
        line_len += copy_bytes(&mut line[line_len..], b"/");
        line_len += copy_bytes(&mut line[line_len..], state_name(M7_MEMORY_STATE[slot]));
        line_len += copy_bytes(&mut line[line_len..], b"] ");
        line_len += copy_utf8_prefix(&mut line[line_len..], &M7_MEMORY_SUMMARY[slot][..M7_MEMORY_SUMMARY_LEN[slot]]);
        if M7_MEMORY_DETAIL_LEN[slot] != 0 {
            line_len += copy_bytes(&mut line[line_len..], b" | ");
            line_len += copy_utf8_prefix(&mut line[line_len..], &M7_MEMORY_DETAIL[slot][..M7_MEMORY_DETAIL_LEN[slot]]);
        }
        line_len += copy_bytes(&mut line[line_len..], b"\n");
    }
    let take = utf8_safe_prefix_len(&line[..line_len], remaining.min(line_len));
    *idx += copy_bytes(&mut out[*idx..], &line[..take]);
    take
}

fn append_slots_to(out: &mut [u8], idx: &mut usize, slots: &[usize], budget: usize) -> usize {
    let start = *idx;
    let mut slot_index = 0usize;
    while slot_index < slots.len() && *idx - start < budget {
        let slot = slots[slot_index];
        unsafe {
            if M7_MEMORY_KIND[slot] != M7_KIND_NONE && M7_MEMORY_SUMMARY_LEN[slot] != 0 {
                let remaining = budget.saturating_sub(*idx - start);
                if remaining == 0 {
                    break;
                }
                let _ = append_entry_prompt_line(out, idx, slot, remaining);
            }
        }
        slot_index += 1;
    }
    *idx - start
}

fn summary_from_text(out: &mut [u8], text: &[u8]) -> usize {
    let mut len = copy_utf8_prefix(out, text);
    while len != 0 && out[len - 1] == b'\n' {
        len -= 1;
    }
    len
}

fn tool_kind_slot(tool: &[u8]) -> usize {
    if starts_with(tool, tool.len(), b"fetch_url") || starts_with(tool, tool.len(), b"search_web") {
        return M7_SLOT_SOURCE;
    }
    if starts_with(tool, tool.len(), b"list_workspace")
        || starts_with(tool, tool.len(), b"read_file")
        || starts_with(tool, tool.len(), b"write_file")
        || starts_with(tool, tool.len(), b"apply_patch")
    {
        return M7_SLOT_WORKSPACE;
    }
    if starts_with(tool, tool.len(), b"run_process")
        || starts_with(tool, tool.len(), b"read_process_output")
    {
        return M7_SLOT_EXECUTION;
    }
    M7_MEMORY_SLOT_COUNT
}

fn tool_kind_name(tool: &[u8]) -> &'static [u8] {
    let slot = tool_kind_slot(tool);
    match slot {
        M7_SLOT_SOURCE => b"source",
        M7_SLOT_WORKSPACE => b"workspace",
        M7_SLOT_EXECUTION => b"execution",
        _ => b"",
    }
}

fn build_tool_summary(tool: &[u8], result: &[u8], out: &mut [u8]) -> usize {
    let mut idx = 0usize;
    idx += copy_bytes(&mut out[idx..], b"Retained ");
    idx += copy_bytes(&mut out[idx..], tool_kind_name(tool));
    idx += copy_bytes(&mut out[idx..], b" result from ");
    idx += copy_bytes(&mut out[idx..], tool);
    if result.len() != 0 {
        idx += copy_bytes(&mut out[idx..], b".");
    }
    idx
}

fn slot_from_id(id: &[u8]) -> Option<usize> {
    let mut slot = 0usize;
    while slot < M7_MEMORY_SLOT_COUNT {
        unsafe {
            if M7_MEMORY_ID_LEN[slot] == id.len()
                && bytes_eq_local(&M7_MEMORY_ID[slot][..M7_MEMORY_ID_LEN[slot]], id)
            {
                return Some(slot);
            }
        }
        slot += 1;
    }
    None
}

fn kind_matches_filter(slot: usize, kind: &[u8]) -> bool {
    if kind.is_empty() {
        return true;
    }
    unsafe { bytes_eq_local(kind_name(M7_MEMORY_KIND[slot]), kind) }
}

pub(super) fn memory_reset() {
    unsafe {
        M7_MEMORY_STARTED = true;
        M7_MEMORY_TURN = 0;
        let mut slot = 0usize;
        while slot < M7_MEMORY_SLOT_COUNT {
            clear_slot(slot);
            slot += 1;
        }
        M7_BUDGET_INSTRUCTIONS_CHARS = 0;
        M7_BUDGET_CURRENT_REQUEST_CHARS = 0;
        M7_BUDGET_LATEST_TOOL_RESULT_CHARS = 0;
        M7_BUDGET_WORKING_MEMORY_CHARS = 0;
        M7_BUDGET_KNOWN_SOURCES_CHARS = 0;
        M7_BUDGET_WORKSPACE_MEMORY_CHARS = 0;
        M7_BUDGET_SESSION_STATE_CHARS = 0;
        M7_BUDGET_RECENT_CONVERSATION_CHARS = 0;
        M7_BUDGET_ESTIMATED_TOTAL_TOKENS = 0;
    }
}

pub(super) fn note_user_request(text: &[u8]) {
    ensure_started();
    unsafe {
        M7_MEMORY_TURN = M7_MEMORY_TURN.wrapping_add(1);
    }
    let mut summary = [0u8; M7_MEMORY_SUMMARY_MAX];
    let summary_len = summary_from_text(&mut summary, text);
    set_slot(
        M7_SLOT_TASK,
        M7_KIND_TASK,
        M7_STATE_DERIVED,
        b"user_turn",
        &summary[..summary_len],
        b"",
    );
}

pub(super) fn note_tool_result(tool: &[u8], result: &[u8]) {
    ensure_started();
    let slot = tool_kind_slot(tool);
    if slot >= M7_MEMORY_SLOT_COUNT {
        return;
    }
    let kind = match slot {
        M7_SLOT_SOURCE => M7_KIND_SOURCE,
        M7_SLOT_WORKSPACE => M7_KIND_WORKSPACE,
        M7_SLOT_EXECUTION => M7_KIND_EXECUTION,
        _ => M7_KIND_NONE,
    };
    let mut summary = [0u8; M7_MEMORY_SUMMARY_MAX];
    let mut detail = [0u8; M7_MEMORY_DETAIL_MAX];
    let (state, summary_len, detail_len) =
        if let Some((compacted_summary_len, compacted_detail_len)) =
            compact_tool_result_if_needed(tool, result, &mut summary, &mut detail)
        {
            (M7_STATE_COMPACTED, compacted_summary_len, compacted_detail_len)
        } else {
            let raw_state = if slot == M7_SLOT_SOURCE && starts_with(tool, tool.len(), b"search_web") {
                M7_STATE_DERIVED
            } else {
                M7_STATE_RAW
            };
            (
                raw_state,
                build_tool_summary(tool, result, &mut summary),
                summary_from_text(&mut detail, result),
            )
        };
    let previous_state = set_slot(
        slot,
        kind,
        state,
        tool,
        &summary[..summary_len],
        &detail[..detail_len],
    );
    if state == M7_STATE_COMPACTED {
        trace_memory_compacted_local(
            slot,
            previous_state,
            result.len(),
            summary_len + detail_len,
            b"bounded_summary",
        );
    }
}

pub(super) fn note_assistant_response(text: &[u8]) {
    ensure_started();
    let mut summary = [0u8; M7_MEMORY_SUMMARY_MAX];
    let mut detail = [0u8; M7_MEMORY_DETAIL_MAX];
    let (state, summary_len, detail_len) = if text.len() > M7_COMPACTION_TRIGGER_CHARS {
        let (compacted_summary_len, compacted_detail_len) =
            build_conversation_compacted(&mut summary, &mut detail, text);
        (M7_STATE_COMPACTED, compacted_summary_len, compacted_detail_len)
    } else {
        (M7_STATE_DERIVED, summary_from_text(&mut summary, text), 0usize)
    };
    let previous_state = set_slot(
        M7_SLOT_CONVERSATION,
        M7_KIND_CONVERSATION,
        state,
        b"assistant_turn",
        &summary[..summary_len],
        &detail[..detail_len],
    );
    if state == M7_STATE_COMPACTED {
        trace_memory_compacted_local(
            M7_SLOT_CONVERSATION,
            previous_state,
            text.len(),
            summary_len + detail_len,
            b"bounded_summary",
        );
    }
}

pub(super) fn append_working_memory_to(out: &mut [u8], idx: &mut usize, budget: usize) -> usize {
    append_slots_to(out, idx, &[M7_SLOT_TASK, M7_SLOT_EXECUTION, M7_SLOT_CONVERSATION], budget)
}

pub(super) fn append_known_sources_to(out: &mut [u8], idx: &mut usize, budget: usize) -> usize {
    append_slots_to(out, idx, &[M7_SLOT_SOURCE], budget)
}

pub(super) fn append_workspace_memory_to(out: &mut [u8], idx: &mut usize, budget: usize) -> usize {
    append_slots_to(out, idx, &[M7_SLOT_WORKSPACE], budget)
}

pub(super) fn record_context_budget(
    instructions_chars: usize,
    current_request_chars: usize,
    latest_tool_result_chars: usize,
    working_memory_chars: usize,
    known_sources_chars: usize,
    workspace_memory_chars: usize,
    session_state_chars: usize,
    recent_conversation_chars: usize,
) {
    unsafe {
        M7_BUDGET_INSTRUCTIONS_CHARS = instructions_chars;
        M7_BUDGET_CURRENT_REQUEST_CHARS = current_request_chars;
        M7_BUDGET_LATEST_TOOL_RESULT_CHARS = latest_tool_result_chars;
        M7_BUDGET_WORKING_MEMORY_CHARS = working_memory_chars;
        M7_BUDGET_KNOWN_SOURCES_CHARS = known_sources_chars;
        M7_BUDGET_WORKSPACE_MEMORY_CHARS = workspace_memory_chars;
        M7_BUDGET_SESSION_STATE_CHARS = session_state_chars;
        M7_BUDGET_RECENT_CONVERSATION_CHARS = recent_conversation_chars;
        M7_BUDGET_ESTIMATED_TOTAL_TOKENS = estimate_tokens(
            instructions_chars
                + current_request_chars
                + latest_tool_result_chars
                + working_memory_chars
                + known_sources_chars
                + workspace_memory_chars
                + session_state_chars
                + recent_conversation_chars,
        );
    }
    if trace_output_enabled() {
        trace_begin(b"context_budget_snapshot", super::current_trace_step());
        trace_json_u64_field(b"instructions_chars", instructions_chars as u64);
        trace_json_u64_field(b"current_request_chars", current_request_chars as u64);
        trace_json_u64_field(b"latest_tool_result_chars", latest_tool_result_chars as u64);
        trace_json_u64_field(b"working_memory_chars", working_memory_chars as u64);
        trace_json_u64_field(b"known_sources_chars", known_sources_chars as u64);
        trace_json_u64_field(b"workspace_memory_chars", workspace_memory_chars as u64);
        trace_json_u64_field(b"session_state_chars", session_state_chars as u64);
        trace_json_u64_field(b"recent_conversation_chars", recent_conversation_chars as u64);
        trace_json_u64_field(
            b"estimated_total_tokens",
            estimate_tokens(
                instructions_chars
                    + current_request_chars
                    + latest_tool_result_chars
                    + working_memory_chars
                    + known_sources_chars
                    + workspace_memory_chars
                    + session_state_chars
                    + recent_conversation_chars,
            ) as u64,
        );
        uart::write_str("}\n");
    }
}

pub(super) fn build_memory_status_json(out: &mut [u8]) -> usize {
    ensure_started();
    let mut task_count = 0usize;
    let mut source_count = 0usize;
    let mut workspace_count = 0usize;
    let mut execution_count = 0usize;
    let mut conversation_count = 0usize;
    unsafe {
        let mut slot = 0usize;
        while slot < M7_MEMORY_SLOT_COUNT {
            match M7_MEMORY_KIND[slot] {
                M7_KIND_TASK if M7_MEMORY_SUMMARY_LEN[slot] != 0 => task_count += 1,
                M7_KIND_SOURCE if M7_MEMORY_SUMMARY_LEN[slot] != 0 => source_count += 1,
                M7_KIND_WORKSPACE if M7_MEMORY_SUMMARY_LEN[slot] != 0 => workspace_count += 1,
                M7_KIND_EXECUTION if M7_MEMORY_SUMMARY_LEN[slot] != 0 => execution_count += 1,
                M7_KIND_CONVERSATION if M7_MEMORY_SUMMARY_LEN[slot] != 0 => conversation_count += 1,
                _ => {}
            }
            slot += 1;
        }
    }
    let mut idx = 0usize;
    idx += copy_bytes(&mut out[idx..], b"{\"ok\":true,\"counts\":{");
    idx += copy_bytes(&mut out[idx..], b"\"task\":");
    idx += u64_to_dec_local(&mut out[idx..], task_count as u64);
    idx += copy_bytes(&mut out[idx..], b",\"source\":");
    idx += u64_to_dec_local(&mut out[idx..], source_count as u64);
    idx += copy_bytes(&mut out[idx..], b",\"workspace\":");
    idx += u64_to_dec_local(&mut out[idx..], workspace_count as u64);
    idx += copy_bytes(&mut out[idx..], b",\"execution\":");
    idx += u64_to_dec_local(&mut out[idx..], execution_count as u64);
    idx += copy_bytes(&mut out[idx..], b",\"conversation\":");
    idx += u64_to_dec_local(&mut out[idx..], conversation_count as u64);
    idx += copy_bytes(&mut out[idx..], b"},\"budget\":{");
    unsafe {
        idx += copy_bytes(&mut out[idx..], b"\"instructions_chars\":");
        idx += u64_to_dec_local(&mut out[idx..], M7_BUDGET_INSTRUCTIONS_CHARS as u64);
        idx += copy_bytes(&mut out[idx..], b",\"current_request_chars\":");
        idx += u64_to_dec_local(&mut out[idx..], M7_BUDGET_CURRENT_REQUEST_CHARS as u64);
        idx += copy_bytes(&mut out[idx..], b",\"latest_tool_result_chars\":");
        idx += u64_to_dec_local(&mut out[idx..], M7_BUDGET_LATEST_TOOL_RESULT_CHARS as u64);
        idx += copy_bytes(&mut out[idx..], b",\"working_memory_chars\":");
        idx += u64_to_dec_local(&mut out[idx..], M7_BUDGET_WORKING_MEMORY_CHARS as u64);
        idx += copy_bytes(&mut out[idx..], b",\"known_sources_chars\":");
        idx += u64_to_dec_local(&mut out[idx..], M7_BUDGET_KNOWN_SOURCES_CHARS as u64);
        idx += copy_bytes(&mut out[idx..], b",\"workspace_memory_chars\":");
        idx += u64_to_dec_local(&mut out[idx..], M7_BUDGET_WORKSPACE_MEMORY_CHARS as u64);
        idx += copy_bytes(&mut out[idx..], b",\"session_state_chars\":");
        idx += u64_to_dec_local(&mut out[idx..], M7_BUDGET_SESSION_STATE_CHARS as u64);
        idx += copy_bytes(&mut out[idx..], b",\"recent_conversation_chars\":");
        idx += u64_to_dec_local(&mut out[idx..], M7_BUDGET_RECENT_CONVERSATION_CHARS as u64);
        idx += copy_bytes(&mut out[idx..], b",\"estimated_total_tokens\":");
        idx += u64_to_dec_local(&mut out[idx..], M7_BUDGET_ESTIMATED_TOTAL_TOKENS as u64);
    }
    idx += copy_bytes(&mut out[idx..], b"},\"checkpoint_available\":false}");
    idx
}

pub(super) fn build_list_memory_json(kind: &[u8], out: &mut [u8]) -> usize {
    ensure_started();
    let mut idx = 0usize;
    let mut count = 0usize;
    idx += copy_bytes(&mut out[idx..], b"{\"ok\":true,\"entries\":[");
    let mut slot = 0usize;
    while slot < M7_MEMORY_SLOT_COUNT {
        unsafe {
            if M7_MEMORY_KIND[slot] != M7_KIND_NONE
                && M7_MEMORY_SUMMARY_LEN[slot] != 0
                && kind_matches_filter(slot, kind)
            {
                if count != 0 {
                    idx += copy_bytes(&mut out[idx..], b",");
                }
                append_entry_json(out, &mut idx, slot, false);
                count += 1;
            }
        }
        slot += 1;
    }
    idx += copy_bytes(&mut out[idx..], b"],\"truncated\":false}");
    idx
}

pub(super) fn build_read_memory_json(id: &[u8], out: &mut [u8]) -> usize {
    ensure_started();
    let slot = match slot_from_id(id) {
        Some(v) => v,
        None => {
            let mut idx = 0usize;
            idx += copy_bytes(&mut out[idx..], b"{\"ok\":false,\"error\":{\"code\":\"unknown_memory_id\",\"message\":\"memory entry was not found\"}}");
            return idx;
        }
    };
    let mut idx = 0usize;
    idx += copy_bytes(&mut out[idx..], b"{\"ok\":true,\"entry\":");
    append_entry_json(out, &mut idx, slot, true);
    idx += copy_bytes(&mut out[idx..], b"}");
    idx
}

pub(super) fn memory_status_command() {
    clear_inline_status();
    ensure_started();
    let mut buf = [0u8; 1024];
    let len = build_memory_status_json(&mut buf);
    uart::write_bytes(&buf[..len]);
    uart::write_str("\n");
}

pub(super) fn memory_list_command(kind: &[u8]) {
    clear_inline_status();
    ensure_started();
    let mut buf = [0u8; 2048];
    let len = build_list_memory_json(kind, &mut buf);
    uart::write_bytes(&buf[..len]);
    uart::write_str("\n");
}

pub(super) fn memory_read_command(id: &[u8]) {
    clear_inline_status();
    ensure_started();
    let mut buf = [0u8; 2048];
    let len = build_read_memory_json(id, &mut buf);
    uart::write_bytes(&buf[..len]);
    uart::write_str("\n");
}
