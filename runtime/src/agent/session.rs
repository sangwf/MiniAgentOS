use super::*;

const SESSION_HISTORY_MAX: usize = 16_384;
const SESSION_STATE_SLOTS: usize = 8;
const SESSION_STATE_KEY_MAX: usize = 32;
const SESSION_STATE_VALUE_MAX: usize = 2_048;

static mut SESSION_STARTED: bool = false;
static mut SESSION_HISTORY: [u8; SESSION_HISTORY_MAX] = [0u8; SESSION_HISTORY_MAX];
static mut SESSION_HISTORY_LEN: usize = 0;
static mut SESSION_STATE_KEYS: [[u8; SESSION_STATE_KEY_MAX]; SESSION_STATE_SLOTS] =
    [[0u8; SESSION_STATE_KEY_MAX]; SESSION_STATE_SLOTS];
static mut SESSION_STATE_KEY_LENS: [usize; SESSION_STATE_SLOTS] = [0usize; SESSION_STATE_SLOTS];
static mut SESSION_STATE_VALUES: [[u8; SESSION_STATE_VALUE_MAX]; SESSION_STATE_SLOTS] =
    [[0u8; SESSION_STATE_VALUE_MAX]; SESSION_STATE_SLOTS];
static mut SESSION_STATE_VALUE_LENS: [usize; SESSION_STATE_SLOTS] = [0usize; SESSION_STATE_SLOTS];

fn history_append_bytes(bytes: &[u8]) {
    unsafe {
        let remaining = SESSION_HISTORY.len().saturating_sub(SESSION_HISTORY_LEN);
        let take = utf8_safe_prefix_len(bytes, remaining);
        let mut i = 0usize;
        while i < take {
            SESSION_HISTORY[SESSION_HISTORY_LEN] = bytes[i];
            SESSION_HISTORY_LEN += 1;
            i += 1;
        }
    }
}

fn key_matches(slot: usize, key: &[u8]) -> bool {
    unsafe {
        if SESSION_STATE_KEY_LENS[slot] != key.len() {
            return false;
        }
        let mut i = 0usize;
        while i < key.len() {
            if SESSION_STATE_KEYS[slot][i] != key[i] {
                return false;
            }
            i += 1;
        }
    }
    true
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

fn utf8_safe_suffix_start_local(buf: &[u8], keep: usize) -> usize {
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

fn find_slot(key: &[u8]) -> Option<usize> {
    let mut slot = 0usize;
    while slot < SESSION_STATE_SLOTS {
        unsafe {
            if SESSION_STATE_KEY_LENS[slot] != 0 && key_matches(slot, key) {
                return Some(slot);
            }
        }
        slot += 1;
    }
    None
}

fn alloc_slot() -> Option<usize> {
    let mut slot = 0usize;
    while slot < SESSION_STATE_SLOTS {
        unsafe {
            if SESSION_STATE_KEY_LENS[slot] == 0 {
                return Some(slot);
            }
        }
        slot += 1;
    }
    Some(0)
}

pub(super) fn session_started() -> bool {
    unsafe { SESSION_STARTED }
}

pub(super) fn ensure_session_started() {
    if unsafe { SESSION_STARTED } {
        return;
    }
    session_reset();
}

pub(super) fn session_reset() {
    unsafe {
        SESSION_STARTED = true;
        SESSION_HISTORY_LEN = 0;
        let mut slot = 0usize;
        while slot < SESSION_STATE_SLOTS {
            SESSION_STATE_KEY_LENS[slot] = 0;
            SESSION_STATE_VALUE_LENS[slot] = 0;
            slot += 1;
        }
    }
    memory::memory_reset();
    trace_event(b"session_started", 0);
}

pub(super) fn append_user_turn(text: &[u8]) {
    ensure_session_started();
    memory::note_user_request(text);
    history_append_bytes(b"User: ");
    history_append_bytes(text);
    history_append_bytes(b"\n");
}

pub(super) fn append_tool_result(tool: &[u8], result: &[u8]) {
    ensure_session_started();
    memory::note_tool_result(tool, result);
    history_append_bytes(b"Tool[");
    history_append_bytes(tool);
    history_append_bytes(b"]: ");
    history_append_bytes(result);
    history_append_bytes(b"\n");
}

pub(super) fn append_assistant_turn(text: &[u8]) {
    ensure_session_started();
    memory::note_assistant_response(text);
    history_append_bytes(b"Assistant: ");
    history_append_bytes(text);
    history_append_bytes(b"\n");
}

pub(super) fn append_history_to(out: &mut [u8], idx: &mut usize) {
    unsafe {
        *idx += copy_bytes(&mut out[*idx..], &SESSION_HISTORY[..SESSION_HISTORY_LEN]);
    }
}

pub(super) fn append_history_suffix_excluding_current_user_to(
    current_goal: &[u8],
    out: &mut [u8],
    idx: &mut usize,
    budget: usize,
) {
    unsafe {
        let mut effective_len = SESSION_HISTORY_LEN;
        let expected_len = b"User: ".len() + current_goal.len() + 1;
        if effective_len >= expected_len {
            let start = effective_len - expected_len;
            let tail = &SESSION_HISTORY[start..effective_len];
            if starts_with(tail, tail.len(), b"User: ")
                && tail[tail.len() - 1] == b'\n'
                && bytes_eq_local(&tail[b"User: ".len()..tail.len() - 1], current_goal)
            {
                effective_len = start;
            }
        }
        let start = utf8_safe_suffix_start_local(&SESSION_HISTORY[..effective_len], budget);
        *idx += copy_bytes(&mut out[*idx..], &SESSION_HISTORY[start..effective_len]);
    }
}

pub(super) fn append_state_snapshot_to(out: &mut [u8], idx: &mut usize) {
    let mut slot = 0usize;
    unsafe {
        while slot < SESSION_STATE_SLOTS {
            if SESSION_STATE_KEY_LENS[slot] != 0 {
                *idx += copy_bytes(&mut out[*idx..], b"State[");
                *idx += copy_bytes(
                    &mut out[*idx..],
                    &SESSION_STATE_KEYS[slot][..SESSION_STATE_KEY_LENS[slot]],
                );
                *idx += copy_bytes(&mut out[*idx..], b"] = ");
                *idx += copy_bytes(
                    &mut out[*idx..],
                    &SESSION_STATE_VALUES[slot][..SESSION_STATE_VALUE_LENS[slot]],
                );
                *idx += copy_bytes(&mut out[*idx..], b"\n");
            }
            slot += 1;
        }
    }
}

pub(super) fn read_session_state(key: &[u8], out: &mut [u8]) -> usize {
    let slot = match find_slot(key) {
        Some(v) => v,
        None => return 0,
    };
    unsafe { copy_bytes(out, &SESSION_STATE_VALUES[slot][..SESSION_STATE_VALUE_LENS[slot]]) }
}

pub(super) fn write_session_state(key: &[u8], value: &[u8]) -> bool {
    if key.is_empty() || key.len() > SESSION_STATE_KEY_MAX || value.len() > SESSION_STATE_VALUE_MAX {
        return false;
    }
    let slot = match find_slot(key).or_else(alloc_slot) {
        Some(v) => v,
        None => return false,
    };
    unsafe {
        SESSION_STATE_KEY_LENS[slot] = copy_bytes(&mut SESSION_STATE_KEYS[slot], key);
        SESSION_STATE_VALUE_LENS[slot] = copy_utf8_prefix(&mut SESSION_STATE_VALUES[slot], value);
    }
    true
}

pub(super) fn session_status() {
    clear_inline_status();
    ensure_session_started();
    uart::write_str("session active\n");
    uart::write_str("history bytes: ");
    unsafe {
        uart::write_u64_dec(SESSION_HISTORY_LEN as u64);
    }
    uart::write_str("\n");
}
