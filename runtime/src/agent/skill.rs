use super::*;

pub(super) fn agent_emit_terminal_event(step: u8) {
    match unsafe { AGENT_TERMINAL_KIND } {
        AGENT_TERMINAL_COMPLETED => trace_goal_status(b"goal_completed", step, b"ok"),
        AGENT_TERMINAL_REFUSED => trace_goal_status(b"goal_refused", step, b"refused"),
        _ => trace_goal_status(b"goal_failed", step, b"error"),
    }
}

pub(super) fn agent_finish_now() {
    crate::fetch_finish_agent_idle();
    agent_reset();
    uart_prompt();
}

pub(super) fn agent_finish_local(step: u8, terminal_kind: u8) {
    unsafe {
        AGENT_TERMINAL_KIND = terminal_kind;
    }
    clear_inline_status();
    if terminal_kind == AGENT_TERMINAL_COMPLETED && unsafe { AGENT_SUMMARY_LEN } > 0 {
        uart::write_bytes(unsafe { &AGENT_SUMMARY[..AGENT_SUMMARY_LEN] });
        uart::write_str("\n");
    } else if terminal_kind != AGENT_TERMINAL_COMPLETED && unsafe { AGENT_RESULT_REASON_LEN } > 0 {
        uart::write_str("reason: ");
        uart::write_bytes(unsafe { &AGENT_RESULT_REASON[..AGENT_RESULT_REASON_LEN] });
        uart::write_str("\n");
    }
    agent_emit_terminal_event(step);
    agent_finish_now();
}

pub(super) fn agent_start_terminal_post(terminal_kind: u8) {
    unsafe {
        AGENT_TERMINAL_KIND = terminal_kind;
    }
    let sink_url = unsafe { &AGENT_SINK_URL[..AGENT_SINK_URL_LEN] };
    if sink_url.is_empty() {
        agent_finish_local(3, terminal_kind);
        return;
    }
    if unsafe { AGENT_MODE } == AGENT_MODE_M1 && !policy::agent_policy_check(b"post_result", 3, Some(sink_url)) {
        agent_emit_terminal_event(3);
        agent_finish_now();
        return;
    }
    let body_len = unsafe { model::build_agent_result_body(&mut FETCH_BODY) };
    unsafe {
        FETCH_METHOD_POST = true;
        FETCH_BODY_LEN = body_len;
        FETCH_EXTRA_HEADER_LEN = 0;
        FETCH_OAUTH_ACTIVE = false;
    }
    trace_skill_called(b"post_result", 3);
    human_status(b"posting result...");
    let started = fetch_start_agent_url(sink_url, [10, 0, 2, 15], [0, 0, 0, 0], 0);
    if !started {
        trace_skill_result(b"post_result", 3, b"error");
        policy::agent_set_result(b"error", fetch_failure_reason_or(b"post result start failed"));
        agent_emit_terminal_event(3);
        agent_finish_now();
        return;
    }
    unsafe {
        AGENT_PHASE = AGENT_PHASE_POST_RESULT;
    }
}

pub(super) fn agent_fail() {
    clear_inline_status();
    trace_goal_status(b"goal_failed", 3, b"error");
    crate::fetch_finish_agent_idle();
    agent_reset();
    uart_prompt();
}

fn start_m3_openai_summary_request() -> bool {
    let model_url = crate::openai::responses_url();
    if !policy::agent_policy_check(b"call_model", 2, Some(model_url)) {
        if unsafe { AGENT_SINK_URL_LEN } > 0 {
            agent_start_terminal_post(AGENT_TERMINAL_REFUSED);
        } else {
            agent_finish_local(2, AGENT_TERMINAL_REFUSED);
        }
        return false;
    }
    let body_len = unsafe { model::build_openai_summary_request_body(&mut FETCH_BODY) };
    let auth_len = unsafe { crate::openai::build_bearer_header(&mut FETCH_EXTRA_HEADER) };
    if body_len == 0 || auth_len == 0 {
        policy::agent_set_result(b"error", b"openai request build failed");
        if unsafe { AGENT_SINK_URL_LEN } > 0 {
            agent_start_terminal_post(AGENT_TERMINAL_FAILED);
        } else {
            agent_finish_local(2, AGENT_TERMINAL_FAILED);
        }
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
    trace_skill_called(b"call_model", 2);
    human_status(b"summarizing...");
    let started = fetch_start_agent_url(model_url, [10, 0, 2, 15], [0, 0, 0, 0], 0);
    if !started {
        trace_skill_result(b"call_model", 2, b"error");
        policy::agent_set_result(b"error", fetch_failure_reason_or(b"openai request failed"));
        if unsafe { AGENT_SINK_URL_LEN } > 0 {
            agent_start_terminal_post(AGENT_TERMINAL_FAILED);
        } else {
            agent_finish_local(2, AGENT_TERMINAL_FAILED);
        }
        return false;
    }
    unsafe {
        AGENT_PHASE = AGENT_PHASE_CALL_MODEL;
    }
    true
}

pub(super) fn fetch_start_agent_url(url: &[u8], src_ip: [u8; 4], reply_ip: [u8; 4], src_port: u16) -> bool {
    let parts = match parse_agent_url(url, url.len()) {
        Some(v) => v,
        None => {
            set_fetch_error_reason(b"invalid url");
            return false;
        }
    };
    let host = &url[parts.host_start..parts.host_start + parts.host_len];
    let path = if parts.path_len == 0 {
        &[][..]
    } else {
        &url[parts.path_start..parts.path_start + parts.path_len]
    };
    let fixed_ip = if parts.has_fixed_ip {
        Some(parts.fixed_ip)
    } else {
        None
    };
    fetch_start_ex(
        host,
        path,
        src_ip,
        reply_ip,
        src_port,
        parts.https,
        parts.port,
        fixed_ip,
    )
}

pub(crate) fn handle_agent_fetch_done(ok: bool) -> bool {
    if unsafe { !AGENT_TASK_ACTIVE } {
        return false;
    }
    if goal::handle_goal_interpretation_done(ok) {
        return true;
    }
    if unsafe { AGENT_MODE } == AGENT_MODE_M1 {
        if unsafe { AGENT_PHASE } == AGENT_PHASE_FETCH_SOURCE {
            let fetch_ok = model::agent_http_success(ok);
            trace_skill_result(
                b"fetch_url",
                1,
                if fetch_ok { &b"ok"[..] } else { &b"error"[..] },
            );
            if !fetch_ok {
                unsafe {
                    AGENT_SUMMARY_LEN = 0;
                }
                policy::agent_set_result(b"error", b"fetch failed");
                agent_start_terminal_post(AGENT_TERMINAL_FAILED);
                return true;
            }
            let model_url = unsafe { &AGENT_MODEL_URL[..AGENT_MODEL_URL_LEN] };
            if !policy::agent_policy_check(b"call_model", 2, Some(model_url)) {
                agent_start_terminal_post(AGENT_TERMINAL_REFUSED);
                return true;
            }
            let body_len = unsafe { model::build_model_request_body(&mut FETCH_BODY) };
            if body_len == 0 {
                policy::agent_set_result(b"error", b"model request build failed");
                agent_start_terminal_post(AGENT_TERMINAL_FAILED);
                return true;
            }
            unsafe {
                AGENT_RESPONSE_BODY_LEN = 0;
                AGENT_OUTPUT_TEXT_LEN = 0;
                FETCH_METHOD_POST = true;
                FETCH_BODY_LEN = body_len;
                FETCH_EXTRA_HEADER_LEN = 0;
                FETCH_OAUTH_ACTIVE = false;
            }
            trace_skill_called(b"call_model", 2);
            human_status(b"summarizing...");
            let started = fetch_start_agent_url(model_url, [10, 0, 2, 15], [0, 0, 0, 0], 0);
            if !started {
                trace_skill_result(b"call_model", 2, b"error");
                policy::agent_set_result(b"error", fetch_failure_reason_or(b"model request failed"));
                agent_start_terminal_post(AGENT_TERMINAL_FAILED);
                return true;
            }
            unsafe {
                AGENT_PHASE = AGENT_PHASE_CALL_MODEL;
            }
            return true;
        }
        if unsafe { AGENT_PHASE } == AGENT_PHASE_CALL_MODEL {
            let model_ok = model::agent_http_success(ok) && model::capture_model_summary();
            trace_skill_result(
                b"call_model",
                2,
                if model_ok { &b"ok"[..] } else { &b"error"[..] },
            );
            if !model_ok {
                unsafe {
                    AGENT_SUMMARY_LEN = 0;
                }
                policy::agent_set_result(b"error", b"model gateway error");
                agent_start_terminal_post(AGENT_TERMINAL_FAILED);
                return true;
            }
            policy::agent_set_result(b"ok", b"");
            agent_start_terminal_post(AGENT_TERMINAL_COMPLETED);
            return true;
        }
        if unsafe { AGENT_PHASE } == AGENT_PHASE_POST_RESULT {
            let post_ok = model::agent_http_success(ok);
            trace_skill_result(
                b"post_result",
                3,
                if post_ok { &b"ok"[..] } else { &b"error"[..] },
            );
            agent_emit_terminal_event(3);
            agent_finish_now();
            return true;
        }
    }
    if unsafe { AGENT_PHASE } == AGENT_PHASE_FETCH_SOURCE {
        trace_skill_result(
            b"fetch_url",
            1,
            if ok { &b"ok"[..] } else { &b"error"[..] },
        );
        if !ok {
            policy::agent_set_result(b"error", b"fetch failed");
            if unsafe { AGENT_SINK_URL_LEN } > 0 {
                agent_start_terminal_post(AGENT_TERMINAL_FAILED);
            } else {
                agent_finish_local(1, AGENT_TERMINAL_FAILED);
            }
            return true;
        }
        if unsafe { AGENT_MODE } == AGENT_MODE_M3 && crate::openai::api_key_ready() {
            start_m3_openai_summary_request();
            return true;
        }
        if !policy::agent_policy_check(b"summarize_text", 2, None) {
            if unsafe { AGENT_SINK_URL_LEN } > 0 {
                agent_start_terminal_post(AGENT_TERMINAL_REFUSED);
            } else {
                agent_finish_local(2, AGENT_TERMINAL_REFUSED);
            }
            return true;
        }
        trace_skill_called(b"summarize_text", 2);
        let summary_len = unsafe { model::summarize_agent_response(&mut AGENT_SUMMARY, AGENT_SUMMARY_SENTENCES) };
        unsafe { AGENT_SUMMARY_LEN = summary_len; }
        policy::agent_set_result(b"ok", b"");
        trace_skill_result(b"summarize_text", 2, b"ok");
        if unsafe { AGENT_SINK_URL_LEN } > 0 {
            agent_start_terminal_post(AGENT_TERMINAL_COMPLETED);
        } else {
            agent_finish_local(2, AGENT_TERMINAL_COMPLETED);
        }
        return true;
    }
    if unsafe { AGENT_MODE } == AGENT_MODE_M3 && unsafe { AGENT_PHASE } == AGENT_PHASE_CALL_MODEL {
        let model_ok = model::agent_http_success(ok) && model::capture_openai_summary();
        trace_skill_result(
            b"call_model",
            2,
            if model_ok { &b"ok"[..] } else { &b"error"[..] },
        );
        if !model_ok {
            unsafe {
                AGENT_SUMMARY_LEN = 0;
            }
            if model::openai_failure_retryable()
                && unsafe { AGENT_OPENAI_SUMMARY_RETRIES } < AGENT_OPENAI_MAX_RETRIES
            {
                unsafe {
                    AGENT_OPENAI_SUMMARY_RETRIES = AGENT_OPENAI_SUMMARY_RETRIES.wrapping_add(1);
                }
                trace_retry_scheduled(2, b"openai_summary", unsafe {
                    AGENT_OPENAI_SUMMARY_RETRIES
                });
                human_status(b"retrying summary...");
                start_m3_openai_summary_request();
                return true;
            }
            let mut reason = [0u8; 160];
            let reason_len = model::build_openai_failure_reason(&mut reason, b"openai summary");
            policy::agent_set_result(b"error", &reason[..reason_len]);
            if unsafe { AGENT_SINK_URL_LEN } > 0 {
                agent_start_terminal_post(AGENT_TERMINAL_FAILED);
            } else {
                agent_finish_local(2, AGENT_TERMINAL_FAILED);
            }
            return true;
        }
        policy::agent_set_result(b"ok", b"");
        if unsafe { AGENT_SINK_URL_LEN } > 0 {
            agent_start_terminal_post(AGENT_TERMINAL_COMPLETED);
        } else {
            agent_finish_local(2, AGENT_TERMINAL_COMPLETED);
        }
        return true;
    }
    if unsafe { AGENT_PHASE } == AGENT_PHASE_POST_RESULT {
        trace_skill_result(
            b"post_result",
            3,
            if ok { &b"ok"[..] } else { &b"error"[..] },
        );
        if unsafe { AGENT_TERMINAL_KIND } != AGENT_TERMINAL_NONE {
            agent_emit_terminal_event(3);
        } else if ok {
            trace_goal_status(b"goal_completed", 3, b"ok");
        } else {
            trace_goal_status(b"goal_failed", 3, b"error");
        }
        unsafe {
            FETCH_EXTRA_HEADER_LEN = 0;
            FETCH_OAUTH_ACTIVE = false;
            FETCH_STATE = FETCH_IDLE;
        }
        agent_reset();
        uart_prompt();
        return true;
    }
    false
}
