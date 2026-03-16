use super::*;

pub(super) fn start_governed_m1_task() {
    unsafe {
        AGENT_MODE = AGENT_MODE_M1;
        AGENT_PHASE = AGENT_PHASE_FETCH_SOURCE;
        AGENT_RESPONSE_BODY_LEN = 0;
        AGENT_OUTPUT_TEXT_LEN = 0;
        FETCH_METHOD_POST = false;
        FETCH_BODY_LEN = 0;
        FETCH_EXTRA_HEADER_LEN = 0;
        FETCH_OAUTH_ACTIVE = false;
        AGENT_OUTPUT_LANGUAGE_LEN = 0;
        AGENT_OUTPUT_STYLE_LEN = 0;
    }
    policy::agent_set_result(b"ok", b"");
    trace_event(b"plan_created", 0);
    human_status(b"fetching source...");
    let source_url = unsafe { &AGENT_SOURCE_URL[..AGENT_SOURCE_URL_LEN] };
    if !policy::agent_policy_check(b"fetch_url", 1, Some(source_url)) {
        skill::agent_start_terminal_post(AGENT_TERMINAL_REFUSED);
        return;
    }
    trace_skill_called(b"fetch_url", 1);
    let started = skill::fetch_start_agent_url(source_url, [10, 0, 2, 15], [0, 0, 0, 0], 0);
    if !started {
        trace_skill_result(b"fetch_url", 1, b"error");
        unsafe {
            AGENT_SUMMARY_LEN = 0;
        }
        policy::agent_set_result(b"error", fetch_failure_reason_or(b"fetch start failed"));
        skill::agent_start_terminal_post(AGENT_TERMINAL_FAILED);
    }
}

fn start_local_summary_task_with_mode(mode: u8) {
    unsafe {
        AGENT_MODE = mode;
        AGENT_PHASE = AGENT_PHASE_FETCH_SOURCE;
        AGENT_RESPONSE_BODY_LEN = 0;
        AGENT_OUTPUT_TEXT_LEN = 0;
        FETCH_METHOD_POST = false;
        FETCH_BODY_LEN = 0;
        FETCH_EXTRA_HEADER_LEN = 0;
        FETCH_OAUTH_ACTIVE = false;
    }
    policy::agent_set_result(b"ok", b"");
    trace_event(b"plan_created", 0);
    human_status(b"fetching source...");
    let source_url = unsafe { &AGENT_SOURCE_URL[..AGENT_SOURCE_URL_LEN] };
    if !policy::agent_policy_check(b"fetch_url", 1, Some(source_url)) {
        skill::agent_finish_local(1, AGENT_TERMINAL_REFUSED);
        return;
    }
    trace_skill_called(b"fetch_url", 1);
    let started = skill::fetch_start_agent_url(source_url, [10, 0, 2, 15], [0, 0, 0, 0], 0);
    if !started {
        trace_skill_result(b"fetch_url", 1, b"error");
        unsafe {
            AGENT_SUMMARY_LEN = 0;
        }
        policy::agent_set_result(b"error", fetch_failure_reason_or(b"fetch start failed"));
        skill::agent_finish_local(1, AGENT_TERMINAL_FAILED);
    }
}

pub(super) fn start_local_summary_task() {
    start_local_summary_task_with_mode(AGENT_MODE_M2);
}

pub(super) fn start_m3_local_summary_task() {
    start_local_summary_task_with_mode(AGENT_MODE_M3);
}

pub(crate) fn handle_agent_task_line(line: &[u8], len: usize) -> bool {
    if len == 0 || line[0] != b'{' {
        return false;
    }
    if unsafe { AGENT_TASK_ACTIVE || FETCH_STATE != FETCH_IDLE } {
        uart::write_str("busy\n");
        return true;
    }
    let mut kind = [0u8; 64];
    let kind_len = match json_extract_string(line, len, b"kind", &mut kind) {
        Some(v) => v,
        None => {
            uart::write_str("invalid task\n");
            return true;
        }
    };
    let is_m0 = kind_len == b"fetch_summarize_post".len()
        && starts_with(&kind[..], kind_len, b"fetch_summarize_post");
    let is_m1 = kind_len == b"agent_task".len()
        && starts_with(&kind[..], kind_len, b"agent_task");
    if !is_m0 && !is_m1 {
        uart::write_str("unsupported task\n");
        return true;
    }
    let goal_len = match json_extract_string(line, len, b"goal_id", unsafe { &mut AGENT_GOAL_ID }) {
        Some(v) => v,
        None => {
            uart::write_str("missing goal_id\n");
            return true;
        }
    };
    let source_len = match json_extract_string(line, len, b"source_url", unsafe { &mut AGENT_SOURCE_URL }) {
        Some(v) => v,
        None => {
            uart::write_str("missing source_url\n");
            return true;
        }
    };
    let sink_len = match json_extract_string(line, len, b"sink_url", unsafe { &mut AGENT_SINK_URL }) {
        Some(v) => v,
        None => {
            uart::write_str("missing sink_url\n");
            return true;
        }
    };
    unsafe {
        AGENT_TASK_ACTIVE = true;
        AGENT_PHASE = AGENT_PHASE_FETCH_SOURCE;
        AGENT_GOAL_ID_LEN = goal_len;
        AGENT_SOURCE_URL_LEN = source_len;
        AGENT_SINK_URL_LEN = sink_len;
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
    if is_m0 {
        let summary_sentences = json_extract_u64(line, len, b"summary_sentences").unwrap_or(3);
        unsafe {
            AGENT_MODE = AGENT_MODE_M0;
            AGENT_SUMMARY_SENTENCES = if summary_sentences == 0 {
                1
            } else if summary_sentences > 8 {
                8
            } else {
                summary_sentences as usize
            };
        }
        policy::agent_set_result(b"ok", b"");
        trace_event(b"goal_received", 0);
        trace_event(b"intent_parsed", 0);
        trace_event(b"plan_created", 0);
        trace_skill_called(b"fetch_url", 1);
        let started = unsafe {
            skill::fetch_start_agent_url(
                &AGENT_SOURCE_URL[..AGENT_SOURCE_URL_LEN],
                [10, 0, 2, 15],
                [0, 0, 0, 0],
                0,
            )
        };
        if !started {
            trace_skill_result(b"fetch_url", 1, b"error");
            policy::agent_set_result(b"error", fetch_failure_reason_or(b"fetch start failed"));
            skill::agent_fail();
        }
        return true;
    }

    let model_len = match json_extract_string(line, len, b"model_url", unsafe { &mut AGENT_MODEL_URL }) {
        Some(v) => v,
        None => {
            uart::write_str("missing model_url\n");
            agent_reset();
            return true;
        }
    };
    if !policy::agent_store_task_json(line, len) {
        uart::write_str("task too large\n");
        agent_reset();
        return true;
    }
    let max_steps = json_extract_u64(line, len, b"max_steps").unwrap_or(6);
    let max_items = json_extract_u64(line, len, b"max_items").unwrap_or(3);
    unsafe {
        AGENT_MODE = AGENT_MODE_M1;
        AGENT_MODEL_URL_LEN = model_len;
        AGENT_MAX_STEPS = if max_steps == 0 { 6 } else { max_steps as usize };
        AGENT_SUMMARY_SENTENCES = if max_items == 0 {
            1
        } else if max_items > 8 {
            8
        } else {
            max_items as usize
        };
    }
    policy::agent_set_result(b"ok", b"");
    trace_event(b"goal_received", 0);
    trace_event(b"intent_parsed", 0);
    start_governed_m1_task();
    true
}
