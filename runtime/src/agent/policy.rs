use super::*;

pub(super) fn agent_store_task_json(line: &[u8], len: usize) -> bool {
    if len > unsafe { AGENT_TASK_JSON.len() } {
        return false;
    }
    unsafe {
        AGENT_TASK_JSON_LEN = copy_bytes(&mut AGENT_TASK_JSON, &line[..len]);
    }
    true
}

pub(super) fn agent_set_result(status: &[u8], reason: &[u8]) {
    unsafe {
        AGENT_RESULT_STATUS_LEN = copy_bytes(&mut AGENT_RESULT_STATUS, status);
        AGENT_RESULT_REASON_LEN = copy_bytes(&mut AGENT_RESULT_REASON, reason);
    }
}

pub(super) fn agent_store_default_goal_policy() -> bool {
    const PROFILE: &[u8] =
        br#"{"allowed_skills":["fetch_url","call_model","post_result"],"allowed_hosts":["10.0.2.2"]}"#;
    agent_store_task_json(PROFILE, PROFILE.len())
}

pub(super) fn agent_store_local_summary_policy() -> bool {
    const PROFILE: &[u8] =
        br#"{"allowed_skills":["fetch_url","summarize_text"],"allowed_hosts":["10.0.2.2"]}"#;
    agent_store_task_json(PROFILE, PROFILE.len())
}

pub(super) fn agent_store_local_post_policy() -> bool {
    const PROFILE: &[u8] = br#"{"allowed_skills":["fetch_url","summarize_text","post_result"],"allowed_hosts":["10.0.2.2"]}"#;
    agent_store_task_json(PROFILE, PROFILE.len())
}

pub(super) fn agent_store_m3_summary_policy() -> bool {
    const PROFILE: &[u8] = br#"{"allowed_skills":["fetch_url","summarize_text","call_model"],"allowed_hosts":["10.0.2.2","api.openai.com"],"allow_public_hosts":true}"#;
    agent_store_task_json(PROFILE, PROFILE.len())
}

pub(super) fn agent_store_m3_post_policy() -> bool {
    const PROFILE: &[u8] = br#"{"allowed_skills":["fetch_url","summarize_text","call_model","post_result"],"allowed_hosts":["10.0.2.2","api.openai.com"],"allow_public_hosts":true}"#;
    agent_store_task_json(PROFILE, PROFILE.len())
}

fn agent_skill_allowed(skill: &[u8]) -> bool {
    unsafe {
        match json_string_array_contains(
            &AGENT_TASK_JSON[..AGENT_TASK_JSON_LEN],
            AGENT_TASK_JSON_LEN,
            b"allowed_skills",
            skill,
        ) {
            Some(v) => v,
            None => true,
        }
    }
}

fn agent_allow_public_hosts() -> bool {
    unsafe {
        json_extract_bool(
            &AGENT_TASK_JSON[..AGENT_TASK_JSON_LEN],
            AGENT_TASK_JSON_LEN,
            b"allow_public_hosts",
        )
        .unwrap_or(false)
    }
}

fn parse_ipv4_octets(host: &[u8]) -> Option<[u8; 4]> {
    let mut octets = [0u8; 4];
    let mut idx = 0usize;
    let mut part = 0usize;
    while part < 4 {
        if idx >= host.len() {
            return None;
        }
        let mut value = 0u16;
        let mut saw = false;
        while idx < host.len() {
            let b = host[idx];
            if !b.is_ascii_digit() {
                break;
            }
            value = value.saturating_mul(10).saturating_add((b - b'0') as u16);
            if value > 255 {
                return None;
            }
            saw = true;
            idx += 1;
        }
        if !saw {
            return None;
        }
        octets[part] = value as u8;
        part += 1;
        if part == 4 {
            break;
        }
        if idx >= host.len() || host[idx] != b'.' {
            return None;
        }
        idx += 1;
    }
    if idx != host.len() {
        return None;
    }
    Some(octets)
}

fn is_private_ipv4(host: &[u8]) -> bool {
    let octets = match parse_ipv4_octets(host) {
        Some(v) => v,
        None => return false,
    };
    octets[0] == 10
        || octets[0] == 127
        || octets[0] == 0
        || (octets[0] == 169 && octets[1] == 254)
        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 168)
}

fn agent_host_allowed(url: &[u8]) -> bool {
    let parts = match parse_agent_url(url, url.len()) {
        Some(v) => v,
        None => return false,
    };
    let host = &url[parts.host_start..parts.host_start + parts.host_len];
    unsafe {
        let explicit = match json_string_array_contains(
            &AGENT_TASK_JSON[..AGENT_TASK_JSON_LEN],
            AGENT_TASK_JSON_LEN,
            b"allowed_hosts",
            host,
        ) {
            Some(v) => v,
            None => return true,
        };
        explicit || (agent_allow_public_hosts() && !is_private_ipv4(host))
    }
}

pub(super) fn agent_policy_check(skill: &[u8], step: u8, target_url: Option<&[u8]>) -> bool {
    let mut allowed = true;
    let mut reason: &[u8] = b"";
    if !agent_skill_allowed(skill) {
        allowed = false;
        reason = b"skill denied by policy";
    } else if let Some(url) = target_url {
        if !agent_host_allowed(url) {
            allowed = false;
            reason = b"host denied by policy";
        }
    }
    let max_steps = unsafe { AGENT_MAX_STEPS };
    if allowed && max_steps != 0 && (step as usize) > max_steps {
        allowed = false;
        reason = b"step budget exceeded";
    }
    trace_policy_checked(skill, step, if allowed { b"ok" } else { b"denied" });
    if !allowed {
        trace_skill_denied(skill, step, reason);
        unsafe {
            AGENT_SUMMARY_LEN = 0;
        }
        agent_set_result(b"refused", reason);
    }
    allowed
}
