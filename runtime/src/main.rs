#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

mod uart;
mod timer;
mod mem;
mod mmio;
mod allocator;
mod virtio;
mod net;
mod tls;
mod oauth;
mod openai;
mod agent;

include!(concat!(env!("OUT_DIR"), "/build_config.rs"));

#[global_allocator]
static ALLOC: allocator::BumpAllocator = allocator::BumpAllocator::new();

const AUTO_FETCH: bool = false;
const AUTO_TLS_LOCAL_FETCH: bool = AUTO_TLS_LOCAL_FETCH_ENABLED;
const AUTO_DOMAIN: &[u8] = b"neverssl.com";
const AUTO_PATH: &[u8] = b"/";
const AUTO_USE_FIXED_IP: bool = false;
const AUTO_FIXED_IP: [u8; 4] = [93, 184, 216, 34];
const XAPI_DOMAIN: &[u8] = b"api.twitter.com";
const XAPI_PATH: &[u8] = b"/2/tweets";
const XAPI_BASE_URL: &[u8] = b"https://api.twitter.com/2/tweets";
const X_SEARCH_RECENT_BASE_URL: &[u8] = b"https://api.twitter.com/2/tweets/search/recent";
const X_SEARCH_RECENT_PATH_PREFIX: &[u8] = b"/2/tweets/search/recent?query=";
const X_SEARCH_RECENT_PATH_SUFFIX: &[u8] =
    b"&max_results=10&tweet.fields=created_at,public_metrics";
const SYNC_DOMAIN: &[u8] = b"httpbin.org";
const SYNC_PATH: &[u8] = b"/get";
const TLS_LOCAL_DOMAIN: &[u8] = b"localhost";
const TLS_LOCAL_IP: [u8; 4] = [10, 0, 2, 2];
const TLS_LOCAL_PATH: &[u8] = b"/";
const TLS_LOCAL_PORT: u16 = 8443;
const M5_BRIDGE_DOMAIN: &[u8] = b"10.0.2.2";
const M5_BRIDGE_IP: [u8; 4] = [10, 0, 2, 2];
const M5_BRIDGE_PORT: u16 = 8090;
const M5_BRIDGE_HEALTH_PATH: &[u8] = b"/healthz";
const M5_BRIDGE_LIST_PREFIX: &[u8] = b"/workspace/list?path=";
const M5_BRIDGE_LIST_SUFFIX: &[u8] = b"&depth=3";
const M5_BRIDGE_READ_PREFIX: &[u8] = b"/workspace/read?path=";
const M5_BRIDGE_READ_SUFFIX: &[u8] = b"&offset=0&limit=1024";
const M5_BRIDGE_WRITE_PATH: &[u8] = b"/workspace/write";
const M5_BRIDGE_APPLY_PATCH_PATH: &[u8] = b"/workspace/apply-patch";
const M5_BRIDGE_RUN_PYTHON_PATH: &[u8] = b"/process/run-python";
const M5_BRIDGE_OUTPUT_PREFIX: &[u8] = b"/process/output?id=";
const M5_BRIDGE_OPENAI_RESPONSES_PATH: &[u8] = b"/openai/responses";
const M5_PYTHON_RUN_TIMEOUT_SEC: u32 = 5;

static mut UDP_REPLY_BUF: [u8; 1600] = [0u8; 1600];
static mut UDP_PAYLOAD_BUF: [u8; 128] = [0u8; 128];
static mut DNS_BUF: [u8; 1536] = [0u8; 1536];
static mut HTTP_BUF: [u8; 98_304] = [0u8; 98_304];
static mut FETCH_DOMAIN: [u8; 256] = [0u8; 256];
static mut FETCH_PATH: [u8; 256] = [0u8; 256];
static mut FETCH_DOMAIN_LEN: usize = 0;
static mut FETCH_PATH_LEN: usize = 0;
static mut FETCH_SRC_IP: [u8; 4] = [0u8; 4];
static mut FETCH_SRC_PORT: u16 = 0;
static mut FETCH_TCP_SRC_PORT: u16 = 40000;
static mut NEXT_TCP_PORT: u16 = 40000;
static mut FETCH_BODY: [u8; 65536] = [0u8; 65536];
static mut FETCH_BODY_LEN: usize = 0;
static mut FETCH_METHOD_POST: bool = false;
static mut FETCH_EXTRA_HEADER: [u8; 512] = [0u8; 512];
static mut FETCH_EXTRA_HEADER_LEN: usize = 0;
static mut FETCH_OAUTH_ACTIVE: bool = false;
static mut FETCH_REPLY_IP: [u8; 4] = [0u8; 4];
static mut FETCH_STATE: u8 = 0;
static mut FETCH_HTTPS: bool = false;
static mut FETCH_DST_PORT: u16 = 80;
static mut FETCH_GW_MAC: [u8; 6] = [0u8; 6];
static mut FETCH_HAVE_GW: bool = false;
static mut FETCH_DST_IP: [u8; 4] = [0u8; 4];
static mut FETCH_TARGET_PORT: u16 = 0;
static mut FETCH_PROXY: bool = false;
static mut FETCH_SOCKS_SENT: bool = false;
static mut FETCH_SEQ: u32 = 0;
static mut FETCH_ACK: u32 = 0;
static mut FETCH_TCP_ESTABLISHED: bool = false;
static mut FETCH_RETRY: u8 = 0;
static mut FETCH_NEXT_MS: u64 = 0;
static mut FETCH_GOT_RESP: bool = false;
static mut FETCH_DNS_MAC: [u8; 6] = [0u8; 6];
static mut FETCH_HAVE_DNS: bool = false;
static mut FETCH_TX_USED: u16 = 0;
static mut FETCH_TX_INFLIGHT: bool = false;
static mut FETCH_HTTP_SENT: bool = false;
static mut FETCH_HTTP_RETRY: u8 = 0;
static mut FETCH_HTTP_SEQ: u32 = 0;
static mut FETCH_HTTP_LEN: u16 = 0;
static mut FETCH_ACK_SENT: bool = false;
static mut FETCH_DEADLINE_MS: u64 = 0;
const FETCH_MAX_RETRY: u8 = 5;
const FETCH_MAX_ROUNDS: u8 = 2;
const FETCH_TRANSPORT_COOLDOWN_MS: u64 = 1_500;
const FETCH_TRANSPORT_SUCCESS_COOLDOWN_MS: u64 = 800;
static mut FETCH_ROUNDS: u8 = 0;
static mut FETCH_REPLY_SENT: bool = false;
static mut FETCH_REPLY_PENDING: bool = false;
static mut FETCH_PEER_MAC: [u8; 6] = [0u8; 6];
static mut FETCH_HAVE_PEER: bool = false;
static mut FETCH_REPLY_BYTES: usize = 0;
static mut FETCH_CHUNK_IDX: u16 = 0;
const FETCH_MAX_REPLY_BYTES: usize = 4096;
const FETCH_CHUNK_BYTES: usize = 900;
static mut FETCH_REDIRECTS: u8 = 0;
static mut FETCH_REDIRECT_PENDING: bool = false;
static mut FETCH_SUPPRESS_OK: bool = false;
static mut FETCH_REDIRECT_START: bool = false;
static mut FETCH_DONE_PRINTED: bool = false;
static mut FETCH_TRANSPORT_COOLDOWN_UNTIL_MS: u64 = 0;
static mut BODY_REPLY_BYTES: usize = 0;
static mut BODY_CHUNK_IDX: u16 = 0;
static mut OAUTH_EPOCH_SEC: u64 = 0;
static mut OAUTH_EPOCH_TICKS: u64 = 0;
static mut OAUTH_NONCE_COUNTER: u64 = 0;

static mut HTTP_HEADER_BUF: [u8; 4096] = [0u8; 4096];
static mut HTTP_HEADER_LEN: usize = 0;
static mut HTTP_STATUS: u16 = 0;
static mut HTTP_IS_CHUNKED: bool = false;
static mut HTTP_CONTENT_LEN: usize = 0;
static mut HTTP_BODY_RECV: usize = 0;
static mut HTTP_PARSE_STATE: u8 = 0;
static mut HTTP_CHUNK_REMAIN: usize = 0;
static mut HTTP_CHUNK_PARSE: usize = 0;
static mut HTTP_CHUNK_HAVE_DIGIT: bool = false;
static mut HTTP_CHUNK_EXT: bool = false;
static mut HTTP_CHUNK_EXPECT_LF: bool = false;
static mut HTTP_IS_JSON: bool = false;
static mut HTTP_STATUS_SENT: bool = false;
static mut UART_PRINT_HEADERS: bool = false;
static mut UART_PRINT_BODY: bool = false;
static mut UART_PRINT_JSON: bool = false;

const DEBUG_NET: bool = false;
static mut FETCH_REDIR_DOMAIN: [u8; 256] = [0u8; 256];
static mut FETCH_REDIR_DOMAIN_LEN: usize = 0;
static mut FETCH_REDIR_PATH: [u8; 256] = [0u8; 256];
static mut FETCH_REDIR_PATH_LEN: usize = 0;
static mut FETCH_REDIR_HTTPS: bool = false;
const FETCH_MAX_REDIRECTS: u8 = 3;

const PROXY_SOCKS5: bool = HOST_SOCKS5_PROXY_ENABLED;
const NATIVE_OPENAI_REUSE: bool = NATIVE_OPENAI_TRANSPORT_REUSE_ENABLED;
const PROXY_IP: [u8; 4] = [10, 0, 2, 2];
const PROXY_PORT: u16 = HOST_SOCKS5_PROXY_PORT;
static mut TLS_HTTP_LEN: usize = 0;
static mut TLS_HTTP_OFF: usize = 0;
static mut TLS_TCP_LOGS: u8 = 0;
static mut TLS_CERT_LOGS: u8 = 0;
static mut UART_LINE_BUF: [u8; 2048] = [0u8; 2048];
static mut UART_CLEAN_LINE_BUF: [u8; 2048] = [0u8; 2048];
static mut UART_LINE_LEN: usize = 0;
static mut UART_PROMPT: bool = false;
static mut UART_PROMPT_COUNT: u64 = 0;
static mut UART_INPUT_ESCAPE_ACTIVE: bool = false;
static mut UART_INPUT_COLOR_ACTIVE: bool = false;
const UART_LINE_MAX: usize = 2048;
static mut FETCH_FIXED_IP: [u8; 4] = [0u8; 4];
static mut FETCH_HAVE_FIXED_IP: bool = false;
static mut FETCH_ERROR_REASON: [u8; 160] = [0u8; 160];
static mut FETCH_ERROR_REASON_LEN: usize = 0;
static mut FETCH_TRACE_LAST_STATE: u8 = 0xff;
static mut FETCH_PEER_CLOSED: bool = false;
static mut FETCH_OPENAI_REQUEST: bool = false;
static mut FETCH_OPENAI_REUSABLE: bool = false;
static mut FETCH_DEBUG_START_CALLS: u32 = 0;
static mut FETCH_DEBUG_SET_HTTPS: bool = false;
static mut FETCH_DEBUG_SET_HTTPS_BYTE: u8 = 0;
static mut FETCH_DEBUG_SET_PATH_LEN: usize = 0;
static mut FETCH_DEBUG_POST_RESET_HTTPS: bool = false;
static mut FETCH_DEBUG_POST_RESET_HTTPS_BYTE: u8 = 0;
static mut FETCH_DEBUG_POST_RESET_PATH_LEN: usize = 0;
static mut FETCH_DEBUG_POST_STATE_HTTPS: bool = false;
static mut FETCH_DEBUG_POST_STATE_HTTPS_BYTE: u8 = 0;
static mut FETCH_DEBUG_POST_STATE_PATH_LEN: usize = 0;
static mut FETCH_DEBUG_FINAL_HTTPS: bool = false;
static mut FETCH_DEBUG_FINAL_HTTPS_BYTE: u8 = 0;
static mut FETCH_DEBUG_SYNACK_HTTPS: bool = false;
static mut FETCH_DEBUG_SYNACK_PATH_LEN: usize = 0;
static mut FETCH_DEBUG_LAST_ROUNDS_SET: u8 = 0;
const DNS_CACHE_SLOTS: usize = 8;
const DNS_CACHE_TTL_MS: u64 = 60_000;
static mut DNS_CACHE_NAMES: [[u8; 256]; DNS_CACHE_SLOTS] = [[0u8; 256]; DNS_CACHE_SLOTS];
static mut DNS_CACHE_NAME_LENS: [usize; DNS_CACHE_SLOTS] = [0usize; DNS_CACHE_SLOTS];
static mut DNS_CACHE_IPS: [[u8; 4]; DNS_CACHE_SLOTS] = [[0u8; 4]; DNS_CACHE_SLOTS];
static mut DNS_CACHE_EXPIRY_MS: [u64; DNS_CACHE_SLOTS] = [0u64; DNS_CACHE_SLOTS];
static mut DNS_CACHE_VALID: [bool; DNS_CACHE_SLOTS] = [false; DNS_CACHE_SLOTS];
static mut DNS_CACHE_NEXT: usize = 0;
static mut UI_TRACE_ENABLED: bool = false;
static mut UI_DEBUG_ENABLED: bool = false;
static mut UI_STATUS_INLINE: bool = true;
static mut UI_STATUS_ACTIVE: bool = false;
static mut NET_IFACE_READY: bool = false;
static mut NET_IFACE_NB: usize = 0;
static mut NET_IFACE_MAC: [u8; 6] = [0u8; 6];

static mut AGENT_MODE: u8 = 0;
static mut AGENT_TASK_ACTIVE: bool = false;
static mut AGENT_PHASE: u8 = 0;
static mut AGENT_GOAL_ID: [u8; 96] = [0u8; 96];
static mut AGENT_GOAL_ID_LEN: usize = 0;
static mut AGENT_GOAL_TEXT: [u8; 768] = [0u8; 768];
static mut AGENT_GOAL_TEXT_LEN: usize = 0;
static mut AGENT_SOURCE_URL: [u8; 256] = [0u8; 256];
static mut AGENT_SOURCE_URL_LEN: usize = 0;
static mut AGENT_SINK_URL: [u8; 256] = [0u8; 256];
static mut AGENT_SINK_URL_LEN: usize = 0;
static mut AGENT_MODEL_URL: [u8; 256] = [0u8; 256];
static mut AGENT_MODEL_URL_LEN: usize = 0;
static mut AGENT_TASK_JSON: [u8; 1536] = [0u8; 1536];
static mut AGENT_TASK_JSON_LEN: usize = 0;
static mut AGENT_MAX_STEPS: usize = 0;
static mut AGENT_SUMMARY: [u8; 4096] = [0u8; 4096];
static mut AGENT_SUMMARY_LEN: usize = 0;
static mut AGENT_RESPONSE_BODY: [u8; 65536] = [0u8; 65536];
static mut AGENT_RESPONSE_BODY_LEN: usize = 0;
static mut AGENT_RESPONSE_BODY_TRUNCATED: bool = false;
static mut AGENT_OUTPUT_TEXT: [u8; 16384] = [0u8; 16384];
static mut AGENT_OUTPUT_TEXT_LEN: usize = 0;
static mut AGENT_SUMMARY_SENTENCES: usize = 3;
static mut AGENT_OUTPUT_LANGUAGE: [u8; 16] = [0u8; 16];
static mut AGENT_OUTPUT_LANGUAGE_LEN: usize = 0;
static mut AGENT_OUTPUT_STYLE: [u8; 16] = [0u8; 16];
static mut AGENT_OUTPUT_STYLE_LEN: usize = 0;
static mut AGENT_RESULT_STATUS: [u8; 16] = [0u8; 16];
static mut AGENT_RESULT_STATUS_LEN: usize = 0;
static mut AGENT_RESULT_REASON: [u8; 128] = [0u8; 128];
static mut AGENT_RESULT_REASON_LEN: usize = 0;
static mut AGENT_TERMINAL_KIND: u8 = 0;
static mut AGENT_OPENAI_INTERPRET_RETRIES: u8 = 0;
static mut AGENT_OPENAI_SUMMARY_RETRIES: u8 = 0;

const FETCH_IDLE: u8 = 0;
const FETCH_ARP: u8 = 1;
const FETCH_DNS: u8 = 2;
const FETCH_SYN: u8 = 3;
const FETCH_HTTP: u8 = 4;
const FETCH_TLS_HANDSHAKE: u8 = 5;
const FETCH_TLS_HTTP: u8 = 6;
const FETCH_TLS_READ: u8 = 7;
const FETCH_DONE: u8 = 8;
const FETCH_SOCKS_HELLO: u8 = 9;
const FETCH_SOCKS_CONNECT: u8 = 10;
const AGENT_MODE_NONE: u8 = 0;
const AGENT_MODE_M0: u8 = 1;
const AGENT_MODE_M1: u8 = 2;
const AGENT_MODE_M2: u8 = 3;
const AGENT_MODE_M3: u8 = 4;
const AGENT_MODE_M4: u8 = 5;
const AGENT_PHASE_IDLE: u8 = 0;
const AGENT_PHASE_FETCH_SOURCE: u8 = 1;
const AGENT_PHASE_CALL_MODEL: u8 = 2;
const AGENT_PHASE_POST_RESULT: u8 = 3;
const AGENT_PHASE_INTERPRET_GOAL: u8 = 4;
const AGENT_PHASE_M4_MODEL: u8 = 5;
const AGENT_PHASE_M4_FETCH_URL: u8 = 6;
const AGENT_PHASE_M4_POST_URL: u8 = 7;
const AGENT_PHASE_M4_POST_TWEET: u8 = 8;
const AGENT_PHASE_M4_SEARCH_RECENT: u8 = 9;
const AGENT_PHASE_M4_GET_USER_POSTS: u8 = 10;
const AGENT_PHASE_M4_SUMMARY_MODEL: u8 = 11;
const AGENT_PHASE_M5_BRIDGE_TOOL: u8 = 12;
const AGENT_TERMINAL_NONE: u8 = 0;
const AGENT_TERMINAL_COMPLETED: u8 = 1;
const AGENT_TERMINAL_REFUSED: u8 = 2;
const AGENT_TERMINAL_FAILED: u8 = 3;
const AGENT_OPENAI_MAX_RETRIES: u8 = 1;

use core::panic::PanicInfo;

fn append_u64_dec(buf: &mut [u8], idx: &mut usize, mut n: u64) {
    if *idx >= buf.len() {
        return;
    }
    if n == 0 {
        buf[*idx] = b'0';
        *idx += 1;
        return;
    }
    let mut tmp = [0u8; 20];
    let mut t = 0usize;
    while n > 0 && t < tmp.len() {
        tmp[t] = b'0' + (n % 10) as u8;
        n /= 10;
        t += 1;
    }
    while t > 0 && *idx < buf.len() {
        t -= 1;
        buf[*idx] = tmp[t];
        *idx += 1;
    }
}

fn append_bytes(buf: &mut [u8], idx: &mut usize, src: &[u8]) {
    let mut i = 0usize;
    while i < src.len() && *idx < buf.len() {
        buf[*idx] = src[i];
        *idx += 1;
        i += 1;
    }
}

fn is_url_unreserved(b: u8) -> bool {
    (b'A'..=b'Z').contains(&b)
        || (b'a'..=b'z').contains(&b)
        || (b'0'..=b'9').contains(&b)
        || b == b'-'
        || b == b'.'
        || b == b'_'
        || b == b'~'
}

fn append_urlencoded(buf: &mut [u8], idx: &mut usize, src: &[u8]) {
    let mut i = 0usize;
    while i < src.len() && *idx < buf.len() {
        let b = src[i];
        if is_url_unreserved(b) {
            buf[*idx] = b;
            *idx += 1;
        } else if *idx + 3 <= buf.len() {
            buf[*idx] = b'%';
            buf[*idx + 1] = b"0123456789ABCDEF"[(b >> 4) as usize];
            buf[*idx + 2] = b"0123456789ABCDEF"[(b & 0x0f) as usize];
            *idx += 3;
        } else {
            return;
        }
        i += 1;
    }
}

fn append_json_escaped(buf: &mut [u8], idx: &mut usize, src: &[u8]) {
    let mut i = 0usize;
    while i < src.len() && *idx < buf.len() {
        let b = src[i];
        match b {
            b'"' | b'\\' => {
                if *idx + 2 > buf.len() {
                    return;
                }
                buf[*idx] = b'\\';
                buf[*idx + 1] = b;
                *idx += 2;
            }
            b'\n' => {
                if *idx + 2 > buf.len() {
                    return;
                }
                buf[*idx] = b'\\';
                buf[*idx + 1] = b'n';
                *idx += 2;
            }
            b'\r' => {
                if *idx + 2 > buf.len() {
                    return;
                }
                buf[*idx] = b'\\';
                buf[*idx + 1] = b'r';
                *idx += 2;
            }
            b'\t' => {
                if *idx + 2 > buf.len() {
                    return;
                }
                buf[*idx] = b'\\';
                buf[*idx + 1] = b't';
                *idx += 2;
            }
            0x00..=0x1f | 0x7f => {
                if *idx + 6 > buf.len() {
                    return;
                }
                buf[*idx] = b'\\';
                buf[*idx + 1] = b'u';
                buf[*idx + 2] = b'0';
                buf[*idx + 3] = b'0';
                buf[*idx + 4] = b"0123456789ABCDEF"[(b >> 4) as usize];
                buf[*idx + 5] = b"0123456789ABCDEF"[(b & 0x0f) as usize];
                *idx += 6;
            }
            _ => {
                buf[*idx] = b;
                *idx += 1;
            }
        }
        i += 1;
    }
}

fn decode_shell_escapes(src: &[u8], out: &mut [u8]) -> usize {
    let mut i = 0usize;
    let mut n = 0usize;
    while i < src.len() && n < out.len() {
        if src[i] != b'\\' || i + 1 >= src.len() {
            out[n] = src[i];
            n += 1;
            i += 1;
            continue;
        }
        i += 1;
        let esc = src[i];
        i += 1;
        out[n] = match esc {
            b'n' => b'\n',
            b'r' => b'\r',
            b't' => b'\t',
            b'\\' => b'\\',
            b'"' => b'"',
            _ => esc,
        };
        n += 1;
    }
    n
}

fn build_m5_list_path(path: &[u8], out: &mut [u8]) -> usize {
    let mut idx = 0usize;
    append_bytes(out, &mut idx, M5_BRIDGE_LIST_PREFIX);
    append_urlencoded(out, &mut idx, path);
    append_bytes(out, &mut idx, M5_BRIDGE_LIST_SUFFIX);
    idx
}

fn build_m5_read_path(path: &[u8], out: &mut [u8]) -> usize {
    let mut idx = 0usize;
    append_bytes(out, &mut idx, M5_BRIDGE_READ_PREFIX);
    append_urlencoded(out, &mut idx, path);
    append_bytes(out, &mut idx, M5_BRIDGE_READ_SUFFIX);
    idx
}

fn build_m5_write_body(path: &[u8], content: &[u8], out: &mut [u8]) -> usize {
    let mut idx = 0usize;
    append_bytes(out, &mut idx, b"{\"path\":\"");
    append_json_escaped(out, &mut idx, path);
    append_bytes(out, &mut idx, b"\",\"content\":\"");
    append_json_escaped(out, &mut idx, content);
    append_bytes(out, &mut idx, b"\",\"create\":true,\"overwrite\":true}");
    idx
}

fn build_m5_patch_body(patch: &[u8], out: &mut [u8]) -> usize {
    let mut idx = 0usize;
    append_bytes(out, &mut idx, b"{\"patch\":\"");
    append_json_escaped(out, &mut idx, patch);
    append_bytes(out, &mut idx, b"\"}");
    idx
}

fn build_m5_run_python_body(path: &[u8], timeout_sec: u32, out: &mut [u8]) -> usize {
    let mut idx = 0usize;
    append_bytes(out, &mut idx, b"{\"path\":\"");
    append_json_escaped(out, &mut idx, path);
    append_bytes(out, &mut idx, b"\",\"timeout_sec\":");
    append_u64_dec(out, &mut idx, timeout_sec as u64);
    append_bytes(out, &mut idx, b"}");
    idx
}

fn build_m5_output_path(process_id: &[u8], out: &mut [u8]) -> usize {
    let mut idx = 0usize;
    append_bytes(out, &mut idx, M5_BRIDGE_OUTPUT_PREFIX);
    append_urlencoded(out, &mut idx, process_id);
    idx
}

fn starts_with(buf: &[u8], len: usize, pat: &[u8]) -> bool {
    if len < pat.len() {
        return false;
    }
    let mut i = 0usize;
    while i < pat.len() {
        if buf[i] != pat[i] {
            return false;
        }
        i += 1;
    }
    true
}

fn starts_with_at(buf: &[u8], len: usize, off: usize, pat: &[u8]) -> bool {
    if off + pat.len() > len {
        return false;
    }
    let mut i = 0usize;
    while i < pat.len() {
        if buf[off + i] != pat[i] {
            return false;
        }
        i += 1;
    }
    true
}

fn is_space(b: u8) -> bool {
    b == b' ' || b == b'\t' || b == b'\r' || b == b'\n'
}

fn is_url_host_char(b: u8) -> bool {
    (b'0'..=b'9').contains(&b)
        || (b'a'..=b'z').contains(&ascii_lower(b))
        || b == b'.'
        || b == b'-'
}

#[derive(Copy, Clone)]
struct UrlParts {
    domain_start: usize,
    domain_len: usize,
    path_start: usize,
    path_len: usize,
    https: bool,
}

#[derive(Copy, Clone)]
struct AgentUrlParts {
    host_start: usize,
    host_len: usize,
    path_start: usize,
    path_len: usize,
    https: bool,
    port: u16,
    has_fixed_ip: bool,
    fixed_ip: [u8; 4],
}

fn parse_url(buf: &[u8], len: usize) -> Option<UrlParts> {
    let mut start = 0usize;
    let mut end = len;
    while start < end && is_space(buf[start]) {
        start += 1;
    }
    while end > start && is_space(buf[end - 1]) {
        end -= 1;
    }
    if start >= end {
        return None;
    }
    if starts_with_at(buf, end, start, b"get ") {
        start += 4;
        while start < end && is_space(buf[start]) {
            start += 1;
        }
    }
    let mut https = false;
    if starts_with_at(buf, end, start, b"http://") {
        start += 7;
    } else if starts_with_at(buf, end, start, b"https://") {
        https = true;
        start += 8;
    }
    if start >= end {
        return None;
    }
    let mut i = start;
    while i < end && !is_space(buf[i]) && buf[i] != b'/' {
        i += 1;
    }
    if i == start {
        return None;
    }
    let domain_start = start;
    let domain_len = i - start;
    let mut dot = false;
    let mut j = 0usize;
    while j < domain_len {
        let c = buf[domain_start + j];
        if c == b'.' {
            dot = true;
        } else if !is_url_host_char(c) {
            return None;
        }
        j += 1;
    }
    if !dot {
        return None;
    }
    if !is_url_host_char(buf[domain_start]) || buf[domain_start] == b'.' || buf[domain_start] == b'-' {
        return None;
    }
    let domain_last = buf[domain_start + domain_len - 1];
    if !is_url_host_char(domain_last) || domain_last == b'.' || domain_last == b'-' {
        return None;
    }
    let mut path_start = 0usize;
    let mut path_len = 0usize;
    if i < end && buf[i] == b'/' {
        let mut p = i;
        while p < end && !is_space(buf[p]) {
            p += 1;
        }
        path_start = i;
        path_len = p - i;
    } else {
        let mut p = i;
        while p < end && is_space(buf[p]) {
            p += 1;
        }
        if p < end && buf[p] == b'/' {
            let mut q = p;
            while q < end && !is_space(buf[q]) {
                q += 1;
            }
            path_start = p;
            path_len = q - p;
        }
    }
    Some(UrlParts {
        domain_start,
        domain_len,
        path_start,
        path_len,
        https,
    })
}

fn parse_ipv4_bytes(buf: &[u8]) -> Option<[u8; 4]> {
    let mut out = [0u8; 4];
    let mut part = 0usize;
    let mut idx = 0usize;
    while part < 4 {
        if idx >= buf.len() {
            return None;
        }
        let mut saw_digit = false;
        let mut value = 0u16;
        while idx < buf.len() {
            let b = buf[idx];
            if !b.is_ascii_digit() {
                break;
            }
            saw_digit = true;
            value = value.saturating_mul(10).saturating_add((b - b'0') as u16);
            if value > 255 {
                return None;
            }
            idx += 1;
        }
        if !saw_digit {
            return None;
        }
        out[part] = value as u8;
        part += 1;
        if part == 4 {
            break;
        }
        if idx >= buf.len() || buf[idx] != b'.' {
            return None;
        }
        idx += 1;
    }
    if idx != buf.len() {
        return None;
    }
    Some(out)
}

fn parse_agent_url(buf: &[u8], len: usize) -> Option<AgentUrlParts> {
    let mut start = 0usize;
    let mut end = len;
    while start < end && is_space(buf[start]) {
        start += 1;
    }
    while end > start && is_space(buf[end - 1]) {
        end -= 1;
    }
    if start >= end {
        return None;
    }
    let mut https = false;
    if starts_with_at(buf, end, start, b"http://") {
        start += 7;
    } else if starts_with_at(buf, end, start, b"https://") {
        https = true;
        start += 8;
    } else {
        return None;
    }
    if start >= end {
        return None;
    }
    let mut host_end = start;
    while host_end < end && !is_space(buf[host_end]) && buf[host_end] != b'/' {
        host_end += 1;
    }
    if host_end == start {
        return None;
    }
    let mut host_len = host_end - start;
    let mut port = if https { 443 } else { 80 };
    let mut colon = host_end;
    while colon > start {
        colon -= 1;
        if buf[colon] == b':' {
            let mut parsed = 0u16;
            let mut saw_digit = false;
            let mut i = colon + 1;
            while i < host_end {
                let b = buf[i];
                if !b.is_ascii_digit() {
                    return None;
                }
                saw_digit = true;
                parsed = parsed
                    .saturating_mul(10)
                    .saturating_add((b - b'0') as u16);
                i += 1;
            }
            if !saw_digit || parsed == 0 {
                return None;
            }
            port = parsed;
            host_len = colon - start;
            break;
        }
    }
    if host_len == 0 {
        return None;
    }
    let mut path_start = host_end;
    let mut path_len = 0usize;
    if host_end < end && buf[host_end] == b'/' {
        let mut p = host_end;
        while p < end && !is_space(buf[p]) {
            p += 1;
        }
        path_start = host_end;
        path_len = p - host_end;
    }
    let host = &buf[start..start + host_len];
    let fixed_ip = parse_ipv4_bytes(host);
    Some(AgentUrlParts {
        host_start: start,
        host_len,
        path_start,
        path_len,
        https,
        port,
        has_fixed_ip: fixed_ip.is_some(),
        fixed_ip: fixed_ip.unwrap_or([0u8; 4]),
    })
}

fn dns_build_query(buf: &mut [u8], name: &[u8], id: u16) -> usize {
    let mut i = 0usize;
    if buf.len() < 12 {
        return 0;
    }
    buf[i] = (id >> 8) as u8; i += 1;
    buf[i] = (id & 0xff) as u8; i += 1;
    buf[i] = 0x01; i += 1; // RD
    buf[i] = 0x00; i += 1;
    buf[i] = 0x00; i += 1; buf[i] = 0x01; i += 1; // QDCOUNT
    buf[i] = 0x00; i += 1; buf[i] = 0x00; i += 1; // ANCOUNT
    buf[i] = 0x00; i += 1; buf[i] = 0x00; i += 1; // NSCOUNT
    buf[i] = 0x00; i += 1; buf[i] = 0x00; i += 1; // ARCOUNT
    let bytes = name;
    let mut start = 0usize;
    let mut idx = 0usize;
    while idx <= bytes.len() {
        if idx == bytes.len() || bytes[idx] == b'.' {
            let len = idx - start;
            if len == 0 || len > 63 || i + 1 + len >= buf.len() {
                return 0;
            }
            let mut j = 0usize;
            while j < len {
                let b = bytes[start + j];
                if !(b'A' <= b && b <= b'Z')
                    && !(b'a' <= b && b <= b'z')
                    && !(b'0' <= b && b <= b'9')
                    && b != b'-'
                {
                    return 0;
                }
                j += 1;
            }
            buf[i] = len as u8; i += 1;
            let mut j = 0usize;
            while j < len {
                buf[i] = bytes[start + j];
                i += 1;
                j += 1;
            }
            start = idx + 1;
        }
        idx += 1;
    }
    if i + 5 > buf.len() {
        return 0;
    }
    buf[i] = 0; i += 1; // end name
    buf[i] = 0x00; i += 1; buf[i] = 0x01; i += 1; // QTYPE A
    buf[i] = 0x00; i += 1; buf[i] = 0x01; i += 1; // QCLASS IN
    i
}

fn dns_skip_name(buf: &[u8], len: usize, mut off: usize) -> Option<usize> {
    let mut seen = 0u32;
    while off < len {
        let b = buf[off];
        if b == 0 {
            return Some(off + 1);
        }
        if (b & 0xc0) == 0xc0 {
            if off + 1 >= len {
                return None;
            }
            off += 2;
            return Some(off);
        }
        let l = b as usize;
        off += 1;
        if off + l > len {
            return None;
        }
        off += l;
        seen += 1;
        if seen > 128 {
            return None;
        }
    }
    None
}

fn dns_parse_response(buf: &[u8], len: usize, id: u16) -> Option<[u8; 4]> {
    if len < 12 {
        return None;
    }
    let rid = ((buf[0] as u16) << 8) | (buf[1] as u16);
    if rid != id {
        return None;
    }
    let flags = ((buf[2] as u16) << 8) | (buf[3] as u16);
    if (flags & 0x0200) != 0 {
        set_fetch_error_reason(b"dns response too large");
        return None;
    }
    if (flags & 0x8000) == 0 || (flags & 0x000f) != 0 {
        return None;
    }
    let qd = ((buf[4] as u16) << 8) | (buf[5] as u16);
    let an = ((buf[6] as u16) << 8) | (buf[7] as u16);
    let mut off = 12usize;
    for _ in 0..qd {
        off = dns_skip_name(buf, len, off)?;
        if off + 4 > len {
            return None;
        }
        off += 4;
    }
    for _ in 0..an {
        off = dns_skip_name(buf, len, off)?;
        if off + 10 > len {
            return None;
        }
        let typ = ((buf[off] as u16) << 8) | (buf[off + 1] as u16);
        let class = ((buf[off + 2] as u16) << 8) | (buf[off + 3] as u16);
        let rdlen = ((buf[off + 8] as u16) << 8) | (buf[off + 9] as u16);
        off += 10;
        if off + rdlen as usize > len {
            return None;
        }
        if typ == 1 && class == 1 && rdlen == 4 {
            return Some([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]);
        }
        off += rdlen as usize;
    }
    None
}

fn dns_cache_name_eq(slot: usize, name: &[u8]) -> bool {
    unsafe {
        if DNS_CACHE_NAME_LENS[slot] != name.len() {
            return false;
        }
        let mut i = 0usize;
        while i < name.len() {
            if ascii_lower(DNS_CACHE_NAMES[slot][i]) != ascii_lower(name[i]) {
                return false;
            }
            i += 1;
        }
    }
    true
}

fn dns_cache_lookup(name: &[u8]) -> Option<[u8; 4]> {
    if name.is_empty() || name.len() > unsafe { DNS_CACHE_NAMES[0].len() } {
        return None;
    }
    let now = timer::ticks_to_ms(timer::counter_ticks(), timer::counter_freq_hz());
    unsafe {
        let mut slot = 0usize;
        while slot < DNS_CACHE_SLOTS {
            if DNS_CACHE_VALID[slot] {
                if DNS_CACHE_EXPIRY_MS[slot] <= now {
                    DNS_CACHE_VALID[slot] = false;
                } else if dns_cache_name_eq(slot, name) {
                    return Some(DNS_CACHE_IPS[slot]);
                }
            }
            slot += 1;
        }
    }
    None
}

fn dns_cache_store(name: &[u8], ip: [u8; 4]) {
    if name.is_empty() || name.len() > unsafe { DNS_CACHE_NAMES[0].len() } {
        return;
    }
    let now = timer::ticks_to_ms(timer::counter_ticks(), timer::counter_freq_hz());
    unsafe {
        let mut slot = 0usize;
        let mut target = None;
        while slot < DNS_CACHE_SLOTS {
            if DNS_CACHE_VALID[slot] && dns_cache_name_eq(slot, name) {
                target = Some(slot);
                break;
            }
            if !DNS_CACHE_VALID[slot] && target.is_none() {
                target = Some(slot);
            }
            slot += 1;
        }
        let slot = target.unwrap_or_else(|| {
            let idx = DNS_CACHE_NEXT % DNS_CACHE_SLOTS;
            DNS_CACHE_NEXT = (DNS_CACHE_NEXT + 1) % DNS_CACHE_SLOTS;
            idx
        });
        let mut i = 0usize;
        while i < name.len() {
            DNS_CACHE_NAMES[slot][i] = ascii_lower(name[i]);
            i += 1;
        }
        DNS_CACHE_NAME_LENS[slot] = name.len();
        DNS_CACHE_IPS[slot] = ip;
        DNS_CACHE_EXPIRY_MS[slot] = now + DNS_CACHE_TTL_MS;
        DNS_CACHE_VALID[slot] = true;
    }
}

fn udp_reply_prefix(buf: &mut [u8], count: u64) -> usize {
    let mut i = 0usize;
    let prefix = b"WalleOS t=";
    let mut k = 0usize;
    while k < prefix.len() && i < buf.len() {
        buf[i] = prefix[k];
        i += 1;
        k += 1;
    }
    let ticks = timer::counter_ticks();
    let ms = timer::ticks_to_ms(ticks, timer::counter_freq_hz());
    append_u64_dec(&mut buf[..], &mut i, ms);
    if i < buf.len() {
        buf[i] = b' ';
        i += 1;
    }
    let cprefix = b"c=";
    k = 0;
    while k < cprefix.len() && i < buf.len() {
        buf[i] = cprefix[k];
        i += 1;
        k += 1;
    }
    append_u64_dec(&mut buf[..], &mut i, count);
    if i < buf.len() {
        buf[i] = b' ';
        i += 1;
    }
    i
}

fn build_http_request(domain: &[u8], path: &[u8], out: &mut [u8]) -> usize {
    if domain.is_empty() {
        return 0;
    }
    let mut i = 0usize;
    let method: &[u8] = unsafe { if FETCH_METHOD_POST { &b"POST "[..] } else { &b"GET "[..] } };
    let path_bytes = if path.is_empty() { b"/" } else { path };
    let mid = b" HTTP/1.1\r\nHost: ";
    let tail_get = b"User-Agent: minios\r\nAccept: */*\r\nAccept-Encoding: identity\r\nConnection: close\r\n";
    let tail_post = b"User-Agent: minios\r\nAccept: */*\r\nAccept-Encoding: identity\r\nContent-Type: application/json\r\nContent-Length: ";
    let body = unsafe { &FETCH_BODY[..FETCH_BODY_LEN] };
    let mut clen_buf = [0u8; 20];
    let mut clen_len = 0usize;
    if unsafe { FETCH_METHOD_POST } {
        let mut v = body.len() as u64;
        let mut tmp = [0u8; 20];
        let mut n = 0usize;
        if v == 0 {
            tmp[0] = b'0';
            n = 1;
        } else {
            while v > 0 && n < tmp.len() {
                tmp[n] = b'0' + (v % 10) as u8;
                v /= 10;
                n += 1;
            }
        }
        let mut j = 0usize;
        while j < n {
            clen_buf[j] = tmp[n - 1 - j];
            j += 1;
        }
        clen_len = n;
    }
    let extra_len = unsafe { FETCH_EXTRA_HEADER_LEN };
    let needed = method.len()
        + path_bytes.len()
        + mid.len()
        + domain.len()
        + 2
        + extra_len
        + if unsafe { FETCH_METHOD_POST } {
            tail_post.len() + clen_len + 4 + body.len()
        } else {
            tail_get.len() + 2
        };
    if needed > out.len() {
        return 0;
    }
    let mut j = 0usize;
    while j < method.len() {
        out[i] = method[j];
        i += 1;
        j += 1;
    }
    j = 0;
    while j < path_bytes.len() {
        out[i] = path_bytes[j];
        i += 1;
        j += 1;
    }
    j = 0;
    while j < mid.len() {
        out[i] = mid[j];
        i += 1;
        j += 1;
    }
    j = 0;
    while j < domain.len() {
        out[i] = domain[j];
        i += 1;
        j += 1;
    }
    out[i] = b'\r';
    i += 1;
    out[i] = b'\n';
    i += 1;
    if extra_len > 0 {
        let hdr = unsafe { &FETCH_EXTRA_HEADER[..extra_len] };
        j = 0;
        while j < hdr.len() {
            out[i] = hdr[j];
            i += 1;
            j += 1;
        }
    }
    if unsafe { FETCH_METHOD_POST } {
        j = 0;
        while j < tail_post.len() {
            out[i] = tail_post[j];
            i += 1;
            j += 1;
        }
        j = 0;
        while j < clen_len {
            out[i] = clen_buf[j];
            i += 1;
            j += 1;
        }
        out[i] = b'\r'; i += 1;
        out[i] = b'\n'; i += 1;
        out[i] = b'\r'; i += 1;
        out[i] = b'\n'; i += 1;
        j = 0;
        while j < body.len() {
            out[i] = body[j];
            i += 1;
            j += 1;
        }
    } else {
        j = 0;
        while j < tail_get.len() {
            out[i] = tail_get[j];
            i += 1;
            j += 1;
        }
        out[i] = b'\r'; i += 1;
        out[i] = b'\n'; i += 1;
    }
    i
}

fn set_oauth_time(epoch_sec: u64) {
    unsafe {
        OAUTH_EPOCH_SEC = epoch_sec;
        OAUTH_EPOCH_TICKS = timer::counter_ticks();
    }
}

fn parse_u64(buf: &[u8], len: usize) -> Option<u64> {
    let mut i = 0usize;
    while i < len && is_space(buf[i]) {
        i += 1;
    }
    if i >= len {
        return None;
    }
    let mut v = 0u64;
    let mut saw = false;
    while i < len {
        let b = buf[i];
        if b.is_ascii_digit() {
            v = v.saturating_mul(10).saturating_add((b - b'0') as u64);
            saw = true;
        } else {
            break;
        }
        i += 1;
    }
    if saw { Some(v) } else { None }
}

fn build_tweet_body(text: &[u8], out: &mut [u8]) -> usize {
    let mut i = 0usize;
    let head = b"{\"text\":\"";
    let tail = b"\"}";
    let mut j = 0usize;
    while j < head.len() && i < out.len() {
        out[i] = head[j];
        i += 1;
        j += 1;
    }
    let mut k = 0usize;
    while k < text.len() && i + 2 < out.len() {
        let b = text[k];
        match b {
            b'\"' | b'\\' => {
                out[i] = b'\\';
                out[i + 1] = b;
                i += 2;
            }
            b'\n' => {
                out[i] = b'\\';
                out[i + 1] = b'n';
                i += 2;
            }
            b'\r' => {
                out[i] = b'\\';
                out[i + 1] = b'r';
                i += 2;
            }
            b'\t' => {
                out[i] = b'\\';
                out[i + 1] = b't';
                i += 2;
            }
            _ => {
                out[i] = b;
                i += 1;
            }
        }
        k += 1;
    }
    j = 0;
    while j < tail.len() && i < out.len() {
        out[i] = tail[j];
        i += 1;
        j += 1;
    }
    i
}

fn prepare_tweet(text: &[u8]) -> bool {
    if !oauth::secrets_ready() {
        return false;
    }
    let body_len = unsafe { build_tweet_body(text, &mut FETCH_BODY) };
    if body_len == 0 {
        return false;
    }
    let timestamp = oauth::now_timestamp(unsafe { OAUTH_EPOCH_SEC }, unsafe { OAUTH_EPOCH_TICKS });
    let nonce = oauth::nonce(unsafe { &mut OAUTH_NONCE_COUNTER });
    let auth_len = unsafe {
        oauth::build_oauth_header(
            b"POST",
            XAPI_BASE_URL,
            timestamp,
            nonce,
            &mut FETCH_EXTRA_HEADER,
        )
    };
    if auth_len == 0 {
        return false;
    }
    unsafe {
        FETCH_METHOD_POST = true;
        FETCH_BODY_LEN = body_len;
        FETCH_EXTRA_HEADER_LEN = auth_len;
        FETCH_OAUTH_ACTIVE = true;
    }
    true
}

// Agent task parsing, policy, skill dispatch, and model handling live under
// `src/agent/` so M1 logic no longer expands as one block in this file.

fn handle_uart_line(line: &[u8], len: usize) {
    let len = sanitize_uart_line(line, len);
    let line = unsafe { &UART_CLEAN_LINE_BUF[..len] };
    if agent::handle_agent_task_line(line, len) {
        return;
    }
    if len == 4 && starts_with(&line[..], len, b"sync") {
        if unsafe { FETCH_STATE } != FETCH_IDLE {
            uart::write_str("busy\n");
            return;
        }
        unsafe {
            FETCH_METHOD_POST = false;
            FETCH_BODY_LEN = 0;
            FETCH_EXTRA_HEADER_LEN = 0;
            FETCH_OAUTH_ACTIVE = false;
        }
        let _ = fetch_start(SYNC_DOMAIN, SYNC_PATH, [10, 0, 2, 15], [0, 0, 0, 0], 0, true);
        uart::write_str("syncing...\n");
        return;
    }
    if len > 5 && starts_with(&line[..], len, b"time ") {
        if let Some(ts) = parse_u64(&line[5..], len - 5) {
            set_oauth_time(ts);
            uart::write_str("time set\n");
            return;
        }
    }
    if len == 8 && starts_with(&line[..], len, b"trace on") {
        clear_inline_status();
        unsafe {
            UI_TRACE_ENABLED = true;
        }
        uart::write_str("trace on\n");
        return;
    }
    if len == 9 && starts_with(&line[..], len, b"trace off") {
        clear_inline_status();
        unsafe {
            UI_TRACE_ENABLED = false;
        }
        uart::write_str("trace off\n");
        return;
    }
    if len == 12 && starts_with(&line[..], len, b"trace status") {
        clear_inline_status();
        if trace_output_enabled() {
            uart::write_str("trace on\n");
        } else {
            uart::write_str("trace off\n");
        }
        return;
    }
    if len == 8 && starts_with(&line[..], len, b"debug on") {
        clear_inline_status();
        unsafe {
            UI_DEBUG_ENABLED = true;
        }
        uart::write_str("debug on\n");
        return;
    }
    if len == 9 && starts_with(&line[..], len, b"debug off") {
        clear_inline_status();
        unsafe {
            UI_DEBUG_ENABLED = false;
        }
        uart::write_str("debug off\n");
        return;
    }
    if len == 12 && starts_with(&line[..], len, b"debug status") {
        clear_inline_status();
        if debug_output_enabled() {
            uart::write_str("debug on\n");
        } else {
            uart::write_str("debug off\n");
        }
        return;
    }
    if len == 13 && starts_with(&line[..], len, b"status inline") {
        clear_inline_status();
        unsafe {
            UI_STATUS_INLINE = true;
        }
        uart::write_str("status inline\n");
        return;
    }
    if len == 12 && starts_with(&line[..], len, b"status plain") {
        clear_inline_status();
        unsafe {
            UI_STATUS_INLINE = false;
        }
        uart::write_str("status plain\n");
        return;
    }
    if len == 13 && starts_with(&line[..], len, b"status status") {
        clear_inline_status();
        if status_inline_enabled() {
            uart::write_str("status inline\n");
        } else {
            uart::write_str("status plain\n");
        }
        return;
    }
    if len == 12 && starts_with(&line[..], len, b"openai-clear") {
        clear_inline_status();
        openai::clear_api_key();
        uart::write_str("openai key cleared\n");
        return;
    }
    if len == 13 && starts_with(&line[..], len, b"openai-status") {
        clear_inline_status();
        if openai::api_key_ready() {
            uart::write_str("openai key ready\n");
        } else {
            uart::write_str("openai key missing\n");
        }
        return;
    }
    if len == 10 && starts_with(&line[..], len, b"tls-status") {
        clear_inline_status();
        print_tls_status();
        return;
    }
    if len == 12 && starts_with(&line[..], len, b"fetch-status") {
        clear_inline_status();
        print_fetch_status();
        return;
    }
    if len == 9 && starts_with(&line[..], len, b"tls-local") {
        clear_inline_status();
        if unsafe { FETCH_STATE } != FETCH_IDLE {
            uart::write_str("busy\n");
            return;
        }
        if start_tls_local_fetch() {
            // Avoid extra UART output after arming fetch so transport state
            // can be observed without the debug path perturbing it.
        } else {
            uart::write_str("tls local fetch failed\n");
        }
        return;
    }
    if len > 11 && starts_with(&line[..], len, b"openai-key ") {
        clear_inline_status();
        let mut start = 11usize;
        while start < len && is_space(line[start]) {
            start += 1;
        }
        let key = if start < len { &line[start..len] } else { &[][..] };
        if openai::set_api_key(key) {
            uart::write_str("openai key stored\n");
        } else {
            uart::write_str("openai key rejected\n");
        }
        return;
    }
    if agent::handle_session_command(line, len) {
        return;
    }
    if len == 9 && starts_with(&line[..], len, b"m5-status") {
        clear_inline_status();
        if unsafe { FETCH_STATE } != FETCH_IDLE {
            uart::write_str("busy\n");
            return;
        }
        if start_m5_bridge_fetch(M5_BRIDGE_HEALTH_PATH) {
            uart::write_str("m5 bridge status...\n");
        } else {
            uart::write_str("m5 bridge fetch failed\n");
        }
        return;
    }
    if (len == 7 && starts_with(&line[..], len, b"m5-list"))
        || (len > 7 && starts_with(&line[..], len, b"m5-list "))
    {
        clear_inline_status();
        if unsafe { FETCH_STATE } != FETCH_IDLE {
            uart::write_str("busy\n");
            return;
        }
        let mut start = 7usize;
        while start < len && is_space(line[start]) {
            start += 1;
        }
        let path = if start < len { &line[start..len] } else { &[][..] };
        let mut path_buf = [0u8; 512];
        let path_len = build_m5_list_path(path, &mut path_buf);
        let started = start_m5_bridge_fetch(&path_buf[..path_len]);
        if started {
            uart::write_str("m5 listing workspace...\n");
        } else {
            uart::write_str("m5 bridge fetch failed\n");
        }
        return;
    }
    if len > 8 && starts_with(&line[..], len, b"m5-read ") {
        clear_inline_status();
        if unsafe { FETCH_STATE } != FETCH_IDLE {
            uart::write_str("busy\n");
            return;
        }
        let mut start = 8usize;
        while start < len && is_space(line[start]) {
            start += 1;
        }
        if start >= len {
            uart::write_str("usage: m5-read <path>\n");
            return;
        }
        let path = &line[start..len];
        let mut path_buf = [0u8; 512];
        let path_len = build_m5_read_path(path, &mut path_buf);
        let started = start_m5_bridge_fetch(&path_buf[..path_len]);
        if started {
            uart::write_str("m5 reading file...\n");
        } else {
            uart::write_str("m5 bridge fetch failed\n");
        }
        return;
    }
    if len > 9 && starts_with(&line[..], len, b"m5-write ") {
        clear_inline_status();
        if unsafe { FETCH_STATE } != FETCH_IDLE {
            uart::write_str("busy\n");
            return;
        }
        let mut start = 9usize;
        while start < len && is_space(line[start]) {
            start += 1;
        }
        if start >= len {
            uart::write_str("usage: m5-write <path> <content>\n");
            return;
        }
        let mut path_end = start;
        while path_end < len && !is_space(line[path_end]) {
            path_end += 1;
        }
        let path = &line[start..path_end];
        let mut content_start = path_end;
        while content_start < len && is_space(line[content_start]) {
            content_start += 1;
        }
        let raw_content = if content_start < len {
            &line[content_start..len]
        } else {
            &[][..]
        };
        let mut content_buf = [0u8; 1024];
        let content_len = decode_shell_escapes(raw_content, &mut content_buf);
        let mut body_buf = [0u8; 1536];
        let body_len = build_m5_write_body(path, &content_buf[..content_len], &mut body_buf);
        if body_len == 0 {
            uart::write_str("m5 write body too large\n");
            return;
        }
        let started = start_m5_bridge_post(M5_BRIDGE_WRITE_PATH, &body_buf[..body_len]);
        if started {
            uart::write_str("m5 writing file...\n");
        } else {
            uart::write_str("m5 bridge fetch failed\n");
        }
        return;
    }
    if len > 15 && starts_with(&line[..], len, b"m5-apply-patch ") {
        clear_inline_status();
        if unsafe { FETCH_STATE } != FETCH_IDLE {
            uart::write_str("busy\n");
            return;
        }
        let mut start = 15usize;
        while start < len && is_space(line[start]) {
            start += 1;
        }
        if start >= len {
            uart::write_str("usage: m5-apply-patch <escaped-patch>\n");
            return;
        }
        let raw_patch = &line[start..len];
        let mut patch_buf = [0u8; 2048];
        let patch_len = decode_shell_escapes(raw_patch, &mut patch_buf);
        let mut body_buf = [0u8; 3072];
        let body_len = build_m5_patch_body(&patch_buf[..patch_len], &mut body_buf);
        if body_len == 0 {
            uart::write_str("m5 patch body too large\n");
            return;
        }
        let started = start_m5_bridge_post(M5_BRIDGE_APPLY_PATCH_PATH, &body_buf[..body_len]);
        if started {
            uart::write_str("m5 applying patch...\n");
        } else {
            uart::write_str("m5 bridge fetch failed\n");
        }
        return;
    }
    if len > 7 && starts_with(&line[..], len, b"m5-run ") {
        clear_inline_status();
        if unsafe { FETCH_STATE } != FETCH_IDLE {
            uart::write_str("busy\n");
            return;
        }
        let mut start = 7usize;
        while start < len && is_space(line[start]) {
            start += 1;
        }
        if start >= len {
            uart::write_str("usage: m5-run <path>\n");
            return;
        }
        let path = &line[start..len];
        let mut body_buf = [0u8; 512];
        let body_len = build_m5_run_python_body(path, M5_PYTHON_RUN_TIMEOUT_SEC, &mut body_buf);
        if body_len == 0 {
            uart::write_str("m5 run body too large\n");
            return;
        }
        let started = start_m5_bridge_post(M5_BRIDGE_RUN_PYTHON_PATH, &body_buf[..body_len]);
        if started {
            uart::write_str("m5 running python...\n");
        } else {
            uart::write_str("m5 bridge fetch failed\n");
        }
        return;
    }
    if len > 10 && starts_with(&line[..], len, b"m5-output ") {
        clear_inline_status();
        if unsafe { FETCH_STATE } != FETCH_IDLE {
            uart::write_str("busy\n");
            return;
        }
        let mut start = 10usize;
        while start < len && is_space(line[start]) {
            start += 1;
        }
        if start >= len {
            uart::write_str("usage: m5-output <process-id>\n");
            return;
        }
        let process_id = &line[start..len];
        let mut path_buf = [0u8; 256];
        let path_len = build_m5_output_path(process_id, &mut path_buf);
        let started = start_m5_bridge_fetch(&path_buf[..path_len]);
        if started {
            uart::write_str("m5 reading process output...\n");
        } else {
            uart::write_str("m5 bridge fetch failed\n");
        }
        return;
    }
    if (len > 6 && starts_with(&line[..], len, b"tweet "))
        || (len > 11 && starts_with(&line[..], len, b"post_tweet "))
    {
        let mut start = if len > 11 && starts_with(&line[..], len, b"post_tweet ") {
            11usize
        } else {
            6usize
        };
        while start < len && is_space(line[start]) {
            start += 1;
        }
        let text = if start < len { &line[start..len] } else { &[][..] };
        if unsafe { FETCH_STATE } != FETCH_IDLE {
            uart::write_str("busy\n");
            return;
        }
        if prepare_tweet(text) {
            let _ = fetch_start(XAPI_DOMAIN, XAPI_PATH, [10, 0, 2, 15], [0, 0, 0, 0], 0, true);
            uart::write_str("tweeting...\n");
        } else {
            uart::write_str("tweet setup failed\n");
        }
        return;
    }
    if len > 5 && starts_with(&line[..], len, b"post ") {
        let mut start = 5usize;
        while start < len && is_space(line[start]) {
            start += 1;
        }
        let mut end = start;
        while end < len && !is_space(line[end]) {
            end += 1;
        }
        if start < end {
            let url_slice = &line[start..end];
            if let Some(url) = parse_url(url_slice, url_slice.len()) {
                let mut body_start = end;
                while body_start < len && is_space(line[body_start]) {
                    body_start += 1;
                }
                let body = if body_start < len { &line[body_start..len] } else { &[][..] };
                let domain = &url_slice[url.domain_start..url.domain_start + url.domain_len];
                let path = if url.path_len == 0 {
                    &[][..]
                } else {
                    &url_slice[url.path_start..url.path_start + url.path_len]
                };
                unsafe {
                    FETCH_METHOD_POST = true;
                    FETCH_EXTRA_HEADER_LEN = 0;
                    FETCH_OAUTH_ACTIVE = false;
                    let mut n = body.len();
                    if n > FETCH_BODY.len() {
                        n = FETCH_BODY.len();
                    }
                    let mut i = 0usize;
                    while i < n {
                        FETCH_BODY[i] = body[i];
                        i += 1;
                    }
                    FETCH_BODY_LEN = n;
                }
                uart::write_str("post ");
                uart::write_bytes(domain);
                uart::write_str("\n");
                let _ = fetch_start(domain, path, [10, 0, 2, 15], [0, 0, 0, 0], 0, url.https);
                return;
            }
        }
    }
    if let Some(url) = parse_url(line, len) {
        unsafe {
            FETCH_METHOD_POST = false;
            FETCH_BODY_LEN = 0;
            FETCH_EXTRA_HEADER_LEN = 0;
            FETCH_OAUTH_ACTIVE = false;
        }
        if unsafe { FETCH_STATE } != FETCH_IDLE {
            uart::write_str("busy\n");
            return;
        }
        let domain = &line[url.domain_start..url.domain_start + url.domain_len];
        let path = if url.path_len == 0 {
            &[][..]
        } else {
            &line[url.path_start..url.path_start + url.path_len]
        };
        uart::write_str("fetching ");
        uart::write_bytes(domain);
        if url.path_len > 0 {
            uart::write_bytes(path);
        }
        uart::write_str("\n");
        let _ = fetch_start(domain, path, [10, 0, 2, 15], [0, 0, 0, 0], 0, url.https);
        return;
    }
    if (len > 5 && starts_with(&line[..], len, b"goal "))
        || (len > 3 && starts_with(&line[..], len, b"m3 "))
    {
        let mut start = if len > 5 && starts_with(&line[..], len, b"goal ") {
            5usize
        } else {
            3usize
        };
        while start < len && is_space(line[start]) {
            start += 1;
        }
        if start < len && agent::handle_goal_line(&line[start..], len - start) {
            return;
        }
    }
    if agent::handle_m4_goal_line(line, len) {
        return;
    }
    if agent::handle_goal_line(line, len) {
        return;
    }
    if len > 0 {
        uart::write_str("unknown command\n");
    }
}

fn trim_ascii_in_place(buf: &mut [u8], len: usize) -> usize {
    let mut start = 0usize;
    let mut end = len;
    while start < end && is_space(buf[start]) {
        start += 1;
    }
    while end > start && is_space(buf[end - 1]) {
        end -= 1;
    }
    let out_len = end.saturating_sub(start);
    if start != 0 && out_len != 0 {
        let mut i = 0usize;
        while i < out_len {
            buf[i] = buf[start + i];
            i += 1;
        }
    }
    out_len
}

fn is_utf8_continuation_byte(b: u8) -> bool {
    (b & 0b1100_0000) == 0b1000_0000
}

fn utf8_previous_boundary(buf: &[u8], len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let mut idx = len - 1;
    while idx > 0 && is_utf8_continuation_byte(buf[idx]) {
        idx -= 1;
    }
    idx
}

fn uart_begin_input_color() {
    unsafe {
        if UART_INPUT_COLOR_ACTIVE {
            return;
        }
        UART_INPUT_COLOR_ACTIVE = true;
    }
    uart::write_str("\x1b[32m");
}

fn uart_end_input_color() {
    unsafe {
        if !UART_INPUT_COLOR_ACTIVE {
            return;
        }
        UART_INPUT_COLOR_ACTIVE = false;
    }
    uart::write_str("\x1b[0m");
}

fn uart_redraw_input_line() {
    uart_end_input_color();
    clear_inline_status();
    uart::write_str("\r\x1b[2K\r");
    uart::write_str("Goal > ");
    let len = unsafe { UART_LINE_LEN };
    if len != 0 {
        let line = unsafe { &UART_LINE_BUF[..len] };
        uart_begin_input_color();
        uart::write_bytes(line);
    }
}

fn sanitize_uart_line(line: &[u8], len: usize) -> usize {
    let mut out_len = 0usize;
    let mut skip_escape = false;
    let mut i = 0usize;
    unsafe {
        while i < len && out_len < UART_CLEAN_LINE_BUF.len() {
            let b = line[i];
            if skip_escape {
                if (0x40..=0x7e).contains(&b) {
                    skip_escape = false;
                }
                i += 1;
                continue;
            }
            if b == 0x1b {
                skip_escape = true;
                i += 1;
                continue;
            }
            if b < 0x20 || b == 0x7f {
                if b == b'\t' {
                    UART_CLEAN_LINE_BUF[out_len] = b' ';
                    out_len += 1;
                }
                i += 1;
                continue;
            }
            UART_CLEAN_LINE_BUF[out_len] = b;
            out_len += 1;
            i += 1;
        }
        trim_ascii_in_place(&mut UART_CLEAN_LINE_BUF, out_len)
    }
}

fn ascii_lower(b: u8) -> u8 {
    if b'A' <= b && b <= b'Z' {
        b + 32
    } else {
        b
    }
}

fn is_hex(b: u8) -> bool {
    (b'0'..=b'9').contains(&b) || (b'a'..=b'f').contains(&ascii_lower(b))
}

fn hex_val(b: u8) -> u8 {
    let c = ascii_lower(b);
    if c >= b'0' && c <= b'9' {
        c - b'0'
    } else {
        c - b'a' + 10
    }
}

fn header_value(buf: &[u8], len: usize, name: &[u8]) -> Option<(usize, usize)> {
    let mut i = 0usize;
    while i + name.len() + 1 < len {
        let mut ok = true;
        let mut j = 0usize;
        while j < name.len() {
            if ascii_lower(buf[i + j]) != ascii_lower(name[j]) {
                ok = false;
                break;
            }
            j += 1;
        }
        if ok && buf[i + name.len()] == b':' {
            let mut start = i + name.len() + 1;
            while start < len && (buf[start] == b' ' || buf[start] == b'\t') {
                start += 1;
            }
            let mut end = start;
            while end < len && buf[end] != b'\r' && buf[end] != b'\n' {
                end += 1;
            }
            if start < end {
                return Some((start, end));
            }
            return None;
        }
        i += 1;
    }
    None
}

fn parse_status(buf: &[u8], len: usize) -> u16 {
    if len < 12 {
        return 0;
    }
    if !(buf.starts_with(b"HTTP/1.1") || buf.starts_with(b"HTTP/1.0")) {
        return 0;
    }
    let mut i = 8usize;
    while i < len && buf[i] == b' ' {
        i += 1;
    }
    if i + 2 >= len {
        return 0;
    }
    let a = buf[i];
    let b = buf[i + 1];
    let c = buf[i + 2];
    if !(a.is_ascii_digit() && b.is_ascii_digit() && c.is_ascii_digit()) {
        return 0;
    }
    ((a - b'0') as u16) * 100 + ((b - b'0') as u16) * 10 + (c - b'0') as u16
}

fn parse_http_date(buf: &[u8]) -> Option<u64> {
    let mut i = 0usize;
    while i < buf.len() && buf[i] != b',' {
        i += 1;
    }
    if i < buf.len() && buf[i] == b',' {
        i += 1;
    }
    while i < buf.len() && is_space(buf[i]) {
        i += 1;
    }
    if i + 16 >= buf.len() {
        return None;
    }
    let day = parse_2digits(buf, &mut i)?;
    if i < buf.len() && buf[i] == b' ' {
        i += 1;
    }
    let month = parse_month(buf, &mut i)?;
    if i < buf.len() && buf[i] == b' ' {
        i += 1;
    }
    let year = parse_4digits(buf, &mut i)?;
    if i < buf.len() && buf[i] == b' ' {
        i += 1;
    }
    let hour = parse_2digits(buf, &mut i)?;
    if i < buf.len() && buf[i] == b':' {
        i += 1;
    }
    let min = parse_2digits(buf, &mut i)?;
    if i < buf.len() && buf[i] == b':' {
        i += 1;
    }
    let sec = parse_2digits(buf, &mut i)?;
    date_to_epoch(year, month, day, hour, min, sec)
}

fn parse_2digits(buf: &[u8], idx: &mut usize) -> Option<u8> {
    if *idx + 1 >= buf.len() {
        return None;
    }
    let b0 = buf[*idx];
    let b1 = buf[*idx + 1];
    if !b0.is_ascii_digit() || !b1.is_ascii_digit() {
        return None;
    }
    *idx += 2;
    Some(((b0 - b'0') * 10 + (b1 - b'0')) as u8)
}

fn parse_4digits(buf: &[u8], idx: &mut usize) -> Option<u16> {
    if *idx + 3 >= buf.len() {
        return None;
    }
    let mut v = 0u16;
    for _ in 0..4 {
        let b = buf[*idx];
        if !b.is_ascii_digit() {
            return None;
        }
        v = v * 10 + (b - b'0') as u16;
        *idx += 1;
    }
    Some(v)
}

fn parse_month(buf: &[u8], idx: &mut usize) -> Option<u8> {
    if *idx + 2 >= buf.len() {
        return None;
    }
    let a = ascii_lower(buf[*idx]);
    let b = ascii_lower(buf[*idx + 1]);
    let c = ascii_lower(buf[*idx + 2]);
    *idx += 3;
    match (a, b, c) {
        (b'j', b'a', b'n') => Some(1),
        (b'f', b'e', b'b') => Some(2),
        (b'm', b'a', b'r') => Some(3),
        (b'a', b'p', b'r') => Some(4),
        (b'm', b'a', b'y') => Some(5),
        (b'j', b'u', b'n') => Some(6),
        (b'j', b'u', b'l') => Some(7),
        (b'a', b'u', b'g') => Some(8),
        (b's', b'e', b'p') => Some(9),
        (b'o', b'c', b't') => Some(10),
        (b'n', b'o', b'v') => Some(11),
        (b'd', b'e', b'c') => Some(12),
        _ => None,
    }
}

fn tcp_seq_cmp(a: u32, b: u32) -> i32 {
    a.wrapping_sub(b) as i32
}

fn date_to_epoch(year: u16, month: u8, day: u8, hour: u8, min: u8, sec: u8) -> Option<u64> {
    if year < 1970 || month == 0 || month > 12 || day == 0 || day > 31 {
        return None;
    }
    let mut days = 0u64;
    let mut y = 1970u16;
    while y < year {
        days += if is_leap(y) { 366 } else { 365 };
        y += 1;
    }
    let month_days = days_before_month(year, month)?;
    days += month_days as u64;
    days += (day as u64).saturating_sub(1);
    let secs = days * 86400
        + (hour as u64) * 3600
        + (min as u64) * 60
        + (sec as u64);
    Some(secs)
}

fn is_leap(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn days_before_month(year: u16, month: u8) -> Option<u16> {
    let leap = is_leap(year);
    let days = match month {
        1 => 0,
        2 => 31,
        3 => 31 + 28,
        4 => 31 + 28 + 31,
        5 => 31 + 28 + 31 + 30,
        6 => 31 + 28 + 31 + 30 + 31,
        7 => 31 + 28 + 31 + 30 + 31 + 30,
        8 => 31 + 28 + 31 + 30 + 31 + 30 + 31,
        9 => 31 + 28 + 31 + 30 + 31 + 30 + 31 + 31,
        10 => 31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30,
        11 => 31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 31,
        12 => 31 + 28 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 31 + 30,
        _ => return None,
    };
    Some(days + if leap && month > 2 { 1 } else { 0 })
}

fn send_status(nb: usize, mac: [u8; 6], code: u16) {
    let src_ip = unsafe { FETCH_REPLY_IP };
    let src_port = unsafe { FETCH_SRC_PORT };
    if src_port == 0 {
        return;
    }
    let peer_mac = match arp_mac_for(src_ip) {
        Some(m) => m,
        None => return,
    };
    let reply_buf = unsafe { &mut UDP_REPLY_BUF };
    let mut out_len = udp_reply_prefix(reply_buf, 0);
    let prefix = b"http status ";
    let mut j = 0usize;
    while j < prefix.len() && out_len < reply_buf.len() {
        reply_buf[out_len] = prefix[j];
        out_len += 1;
        j += 1;
    }
    let mut tmp = [0u8; 3];
    tmp[0] = b'0' + ((code / 100) as u8);
    tmp[1] = b'0' + (((code / 10) % 10) as u8);
    tmp[2] = b'0' + ((code % 10) as u8);
    j = 0;
    while j < tmp.len() && out_len < reply_buf.len() {
        reply_buf[out_len] = tmp[j];
        out_len += 1;
        j += 1;
    }
    let _ = net::send_udp(nb, mac, [10, 0, 2, 15], 5555, peer_mac, src_ip, src_port, &reply_buf[..out_len]);
}

fn send_body_chunks(nb: usize, mac: [u8; 6], data: &[u8], json: bool) {
    if data.is_empty() {
        return;
    }
    agent::agent_capture_response_body(data);
    unsafe {
        FETCH_GOT_RESP = true;
    }
    if uart_http_dump_enabled() {
        if json {
            if unsafe { !UART_PRINT_JSON } {
                uart::write_str("\n--- json ---\n");
                unsafe { UART_PRINT_JSON = true; }
            }
        } else if unsafe { !UART_PRINT_BODY } {
            uart::write_str("\n--- body ---\n");
            unsafe { UART_PRINT_BODY = true; }
        }
        uart::write_bytes(data);
    }
    let src_ip = unsafe { FETCH_REPLY_IP };
    let src_port = unsafe { FETCH_SRC_PORT };
    if src_port == 0 {
        return;
    }
    let peer_mac = match arp_mac_for(src_ip) {
        Some(m) => m,
        None => return,
    };
    let reply_buf = unsafe { &mut UDP_REPLY_BUF };
    let mut offset = 0usize;
    while offset < data.len() {
        let remaining = FETCH_MAX_REPLY_BYTES.saturating_sub(unsafe { BODY_REPLY_BYTES });
        if remaining == 0 {
            return;
        }
        let mut chunk_len = data.len() - offset;
        if chunk_len > remaining {
            chunk_len = remaining;
        }
        if chunk_len > FETCH_CHUNK_BYTES {
            chunk_len = FETCH_CHUNK_BYTES;
        }
        let mut out_len = udp_reply_prefix(reply_buf, unsafe { BODY_CHUNK_IDX } as u64);
        let prefix = if json { b"json chunk " } else { b"body chunk " };
        let mut j = 0usize;
        while j < prefix.len() && out_len < reply_buf.len() {
            reply_buf[out_len] = prefix[j];
            out_len += 1;
            j += 1;
        }
        let mut idx = unsafe { BODY_CHUNK_IDX };
        let mut digits = [0u8; 5];
        let mut n = 0usize;
        if idx == 0 {
            digits[0] = b'0';
            n = 1;
        } else {
            while idx > 0 && n < digits.len() {
                digits[n] = b'0' + (idx % 10) as u8;
                idx /= 10;
                n += 1;
            }
        }
        let mut k = 0usize;
        while k < n && out_len < reply_buf.len() {
            reply_buf[out_len] = digits[n - 1 - k];
            out_len += 1;
            k += 1;
        }
        if out_len < reply_buf.len() {
            reply_buf[out_len] = b' ';
            out_len += 1;
        }
        let mut m = 0usize;
        while m < chunk_len && out_len < reply_buf.len() {
            reply_buf[out_len] = data[offset + m];
            out_len += 1;
            m += 1;
        }
        let sent1 = net::send_udp(
            nb,
            mac,
            [10, 0, 2, 15],
            5555,
            peer_mac,
            src_ip,
            src_port,
            &reply_buf[..out_len],
        );
        if sent1 {
            let _ = net::send_udp(
                nb,
                mac,
                [10, 0, 2, 15],
                5555,
                peer_mac,
                src_ip,
                src_port,
                &reply_buf[..out_len],
            );
            unsafe {
                BODY_REPLY_BYTES = BODY_REPLY_BYTES.saturating_add(chunk_len);
                BODY_CHUNK_IDX = BODY_CHUNK_IDX.wrapping_add(1);
            }
        } else {
            return;
        }
        offset += chunk_len;
    }
}

fn find_location(buf: &[u8], len: usize) -> Option<(usize, usize)> {
    let mut i = 0usize;
    while i + 9 <= len {
        if ascii_lower(buf[i]) == b'l'
            && ascii_lower(buf[i + 1]) == b'o'
            && ascii_lower(buf[i + 2]) == b'c'
            && ascii_lower(buf[i + 3]) == b'a'
            && ascii_lower(buf[i + 4]) == b't'
            && ascii_lower(buf[i + 5]) == b'i'
            && ascii_lower(buf[i + 6]) == b'o'
            && ascii_lower(buf[i + 7]) == b'n'
            && buf[i + 8] == b':'
        {
            let mut start = i + 9;
            while start < len && (buf[start] == b' ' || buf[start] == b'\t') {
                start += 1;
            }
            let mut end = start;
            while end < len && buf[end] != b'\r' && buf[end] != b'\n' {
                end += 1;
            }
            if start < end {
                return Some((start, end));
            }
            return None;
        }
        i += 1;
    }
    None
}

fn capture_redirect(buf: &[u8], len: usize) {
    if unsafe { FETCH_OAUTH_ACTIVE } {
        return;
    }
    if unsafe { FETCH_REDIRECTS } >= FETCH_MAX_REDIRECTS || unsafe { FETCH_REDIRECT_PENDING } {
        return;
    }
    let loc = match find_location(buf, len) {
        Some(v) => v,
        None => return,
    };
    let val = &buf[loc.0..loc.1];
    let parts = match parse_url(val, val.len()) {
        Some(p) => p,
        None => return,
    };
    unsafe {
        if parts.domain_len == 0 || parts.domain_len > FETCH_REDIR_DOMAIN.len() {
            set_fetch_error_reason(b"redirect url too long");
            return;
        }
        if parts.path_len > FETCH_REDIR_PATH.len() {
            set_fetch_error_reason(b"redirect url too long");
            return;
        }
        let mut i = 0usize;
        while i < parts.domain_len {
            FETCH_REDIR_DOMAIN[i] = val[parts.domain_start + i];
            i += 1;
        }
        FETCH_REDIR_DOMAIN_LEN = parts.domain_len;
        i = 0;
        while i < parts.path_len {
            FETCH_REDIR_PATH[i] = val[parts.path_start + i];
            i += 1;
        }
        FETCH_REDIR_PATH_LEN = parts.path_len;
        FETCH_REDIR_HTTPS = parts.https;
        FETCH_REDIRECT_PENDING = true;
        FETCH_SUPPRESS_OK = true;
        FETCH_REDIRECTS = FETCH_REDIRECTS.wrapping_add(1);
    }
}

fn http_reset() {
    unsafe {
        HTTP_HEADER_LEN = 0;
        HTTP_STATUS = 0;
        HTTP_IS_CHUNKED = false;
        HTTP_CONTENT_LEN = 0;
        HTTP_BODY_RECV = 0;
        HTTP_PARSE_STATE = 0;
        HTTP_CHUNK_REMAIN = 0;
        HTTP_CHUNK_PARSE = 0;
        HTTP_CHUNK_HAVE_DIGIT = false;
        HTTP_CHUNK_EXT = false;
        HTTP_CHUNK_EXPECT_LF = false;
        HTTP_IS_JSON = false;
        HTTP_STATUS_SENT = false;
        BODY_REPLY_BYTES = 0;
        BODY_CHUNK_IDX = 0;
        UART_PRINT_HEADERS = false;
        UART_PRINT_BODY = false;
        UART_PRINT_JSON = false;
    }
}

fn http_parse_headers(buf: &[u8], len: usize) {
    unsafe {
        HTTP_STATUS = parse_status(buf, len);
        if let Some((s, e)) = header_value(buf, len, b"transfer-encoding") {
            let mut i = s;
            while i + 6 <= e {
                if ascii_lower(buf[i]) == b'c'
                    && ascii_lower(buf[i + 1]) == b'h'
                    && ascii_lower(buf[i + 2]) == b'u'
                    && ascii_lower(buf[i + 3]) == b'n'
                    && ascii_lower(buf[i + 4]) == b'k'
                    && ascii_lower(buf[i + 5]) == b'e'
                    && ascii_lower(buf[i + 6]) == b'd'
                {
                    HTTP_IS_CHUNKED = true;
                    break;
                }
                i += 1;
            }
        }
        if let Some((s, e)) = header_value(buf, len, b"content-length") {
            let mut i = s;
            let mut v = 0usize;
            while i < e {
                let b = buf[i];
                if b.is_ascii_digit() {
                    v = v * 10 + (b - b'0') as usize;
                }
                i += 1;
            }
            HTTP_CONTENT_LEN = v;
        }
        if let Some((s, e)) = header_value(buf, len, b"content-type") {
            let mut i = s;
            while i + 15 <= e {
                if ascii_lower(buf[i]) == b'a'
                    && ascii_lower(buf[i + 1]) == b'p'
                    && ascii_lower(buf[i + 2]) == b'p'
                    && ascii_lower(buf[i + 3]) == b'l'
                    && ascii_lower(buf[i + 4]) == b'i'
                    && ascii_lower(buf[i + 5]) == b'c'
                    && ascii_lower(buf[i + 6]) == b'a'
                    && ascii_lower(buf[i + 7]) == b't'
                    && ascii_lower(buf[i + 8]) == b'i'
                    && ascii_lower(buf[i + 9]) == b'o'
                    && ascii_lower(buf[i + 10]) == b'n'
                    && ascii_lower(buf[i + 11]) == b'/'
                    && ascii_lower(buf[i + 12]) == b'j'
                    && ascii_lower(buf[i + 13]) == b's'
                    && ascii_lower(buf[i + 14]) == b'o'
                    && ascii_lower(buf[i + 15]) == b'n'
                {
                    HTTP_IS_JSON = true;
                    break;
                }
                i += 1;
            }
        }
        if let Some((s, e)) = header_value(buf, len, b"date") {
            if let Some(epoch) = parse_http_date(&buf[s..e]) {
                set_oauth_time(epoch);
            }
        }
        capture_redirect(buf, len);
    }
}

fn http_feed(nb: usize, mac: [u8; 6], data: &[u8]) {
    let mut idx = 0usize;
    while idx < data.len() {
        let state = unsafe { HTTP_PARSE_STATE };
        if state == 0 {
            let mut end = None;
            let mut i = idx;
            while i + 3 < data.len() {
                if data[i] == b'\r' && data[i + 1] == b'\n' && data[i + 2] == b'\r' && data[i + 3] == b'\n' {
                    end = Some(i + 4);
                    break;
                }
                i += 1;
            }
            if let Some(h_end) = end {
                let header_len = h_end - idx;
                if header_len > 0 {
                    send_http_chunks(nb, mac, unsafe { FETCH_REPLY_IP }, unsafe { FETCH_SRC_PORT }, &data[idx..h_end]);
                    if uart_http_dump_enabled() {
                        if !unsafe { UART_PRINT_HEADERS } {
                            uart::write_str("\n--- headers ---\n");
                            unsafe { UART_PRINT_HEADERS = true; }
                        }
                        uart::write_bytes(&data[idx..h_end]);
                    }
                    if unsafe { HTTP_HEADER_LEN } + header_len <= unsafe { HTTP_HEADER_BUF.len() } {
                        let mut j = 0usize;
                        while j < header_len {
                            unsafe { HTTP_HEADER_BUF[HTTP_HEADER_LEN + j] = data[idx + j]; }
                            j += 1;
                        }
                        unsafe { HTTP_HEADER_LEN += header_len; }
                    } else {
                        set_fetch_error_reason(b"http headers too large");
                        unsafe {
                            FETCH_GOT_RESP = false;
                            FETCH_STATE = FETCH_DONE;
                        }
                        return;
                    }
                }
                http_parse_headers(unsafe { &HTTP_HEADER_BUF[..HTTP_HEADER_LEN] }, unsafe { HTTP_HEADER_LEN });
                unsafe {
                    if !HTTP_STATUS_SENT && HTTP_STATUS != 0 {
                        send_status(nb, mac, HTTP_STATUS);
                        HTTP_STATUS_SENT = true;
                    }
                    if HTTP_IS_CHUNKED {
                        HTTP_PARSE_STATE = 2;
                    } else {
                        HTTP_PARSE_STATE = 1;
                    }
                }
                idx = h_end;
                continue;
            } else {
                let header_len = data.len() - idx;
                send_http_chunks(nb, mac, unsafe { FETCH_REPLY_IP }, unsafe { FETCH_SRC_PORT }, &data[idx..]);
                if uart_http_dump_enabled() {
                    if !unsafe { UART_PRINT_HEADERS } {
                        uart::write_str("\n--- headers ---\n");
                        unsafe { UART_PRINT_HEADERS = true; }
                    }
                    uart::write_bytes(&data[idx..]);
                }
                if unsafe { HTTP_HEADER_LEN } + header_len <= unsafe { HTTP_HEADER_BUF.len() } {
                    let mut j = 0usize;
                    while j < header_len {
                        unsafe { HTTP_HEADER_BUF[HTTP_HEADER_LEN + j] = data[idx + j]; }
                        j += 1;
                    }
                    unsafe { HTTP_HEADER_LEN += header_len; }
                } else {
                    set_fetch_error_reason(b"http headers too large");
                    unsafe {
                        FETCH_GOT_RESP = false;
                        FETCH_STATE = FETCH_DONE;
                    }
                }
                return;
            }
        } else if state == 1 {
            let mut take = data.len() - idx;
            let limit = unsafe { HTTP_CONTENT_LEN };
            if limit > 0 {
                let remaining = limit.saturating_sub(unsafe { HTTP_BODY_RECV });
                if take > remaining {
                    take = remaining;
                }
            }
            if take == 0 {
                return;
            }
            let json = unsafe { HTTP_IS_JSON };
            send_body_chunks(nb, mac, &data[idx..idx + take], json);
            unsafe { HTTP_BODY_RECV = HTTP_BODY_RECV.saturating_add(take); }
            idx += take;
            if limit > 0 && unsafe { HTTP_BODY_RECV } >= limit {
                unsafe { HTTP_PARSE_STATE = 5; }
                return;
            }
        } else if state == 2 {
            let b = data[idx];
            idx += 1;
            if unsafe { HTTP_CHUNK_EXPECT_LF } {
                if b == b'\n' {
                    let size = unsafe { HTTP_CHUNK_PARSE };
                    unsafe {
                        HTTP_CHUNK_PARSE = 0;
                        HTTP_CHUNK_HAVE_DIGIT = false;
                        HTTP_CHUNK_EXT = false;
                        HTTP_CHUNK_EXPECT_LF = false;
                        HTTP_CHUNK_REMAIN = size;
                        HTTP_PARSE_STATE = if size == 0 { 5 } else { 3 };
                    }
                }
                continue;
            }
            if b == b'\r' {
                unsafe { HTTP_CHUNK_EXPECT_LF = true; }
                continue;
            }
            if unsafe { HTTP_CHUNK_EXT } {
                continue;
            }
            if b == b';' {
                unsafe { HTTP_CHUNK_EXT = true; }
                continue;
            }
            if is_hex(b) {
                unsafe {
                    HTTP_CHUNK_PARSE = (HTTP_CHUNK_PARSE << 4) | (hex_val(b) as usize);
                    HTTP_CHUNK_HAVE_DIGIT = true;
                }
            }
        } else if state == 3 {
            let mut take = data.len() - idx;
            let remain = unsafe { HTTP_CHUNK_REMAIN };
            if take > remain {
                take = remain;
            }
            if take == 0 {
                return;
            }
            let json = unsafe { HTTP_IS_JSON };
            send_body_chunks(nb, mac, &data[idx..idx + take], json);
            unsafe { HTTP_CHUNK_REMAIN = HTTP_CHUNK_REMAIN.saturating_sub(take); }
            idx += take;
            if unsafe { HTTP_CHUNK_REMAIN } == 0 {
                unsafe { HTTP_PARSE_STATE = 4; }
            }
        } else if state == 4 {
            if data[idx] == b'\r' {
                idx += 1;
                if idx < data.len() && data[idx] == b'\n' {
                    idx += 1;
                }
                unsafe { HTTP_PARSE_STATE = 2; }
            } else {
                idx += 1;
            }
        } else {
            return;
        }
    }
}

fn fetch_start_ex(
    domain: &[u8],
    path: &[u8],
    src_ip: [u8; 4],
    reply_ip: [u8; 4],
    src_port: u16,
    https_flag: u8,
    target_port: u16,
    fixed_ip: Option<[u8; 4]>,
) -> bool {
    let domain_len = domain.len();
    let path_len = path.len();
    let https = https_flag != 0;
    let src_ip_value = src_ip;
    let reply_ip_value = reply_ip;
    let src_port_value = src_port;
    let target_port_value = target_port;
    let fixed_ip_present = fixed_ip.is_some();
    let fixed_ip_value = fixed_ip.unwrap_or([0u8; 4]);
    if domain_len == 0 {
        set_fetch_error_reason(b"url host missing");
        return false;
    }
    unsafe {
        if !FETCH_OAUTH_ACTIVE {
            FETCH_EXTRA_HEADER_LEN = 0;
        }
    }
    if domain_len > unsafe { FETCH_DOMAIN.len() } {
        set_fetch_error_reason(b"url host too long");
        return false;
    }
    if path_len > unsafe { FETCH_PATH.len() } {
        set_fetch_error_reason(b"url path too long");
        return false;
    }
    let use_proxy = PROXY_SOCKS5 && https;
    let openai_request = crate::openai::is_responses_target(domain, path, https, target_port_value);
    let can_reuse_openai =
        use_proxy && fetch_openai_reuse_candidate(domain, path, https, target_port_value);
    if !can_reuse_openai && unsafe { FETCH_OPENAI_REUSABLE } {
        fetch_close_current_transport();
    }
    let mut have_gw = false;
    let mut gw_mac = [0u8; 6];
    let mut have_dns = false;
    let mut dns_mac = [0u8; 6];
    if let Some(m) = net::lookup_arp_peer([10, 0, 2, 2]) {
        gw_mac = m;
        have_gw = true;
        agent::trace_fetch_cache_hit(b"arp", b"gateway");
    }
    if let Some(m) = net::lookup_arp_peer([10, 0, 2, 3]) {
        dns_mac = m;
        have_dns = true;
        agent::trace_fetch_cache_hit(b"arp", b"dns");
    }
    // QEMU user-mode networking exposes DNS at 10.0.2.3, but the packet still
    // egresses through the same virtual gateway MAC as 10.0.2.2. If we already
    // know the gateway MAC, treat it as a safe fallback for DNS as well.
    if !have_dns && have_gw {
        dns_mac = gw_mac;
        have_dns = true;
    }
    unsafe {
        if use_proxy && !have_gw && FETCH_HAVE_GW {
            gw_mac = FETCH_GW_MAC;
            have_gw = true;
        }
    }
    let cached_dns_ip = if !use_proxy && !fixed_ip_present && !AUTO_USE_FIXED_IP {
        dns_cache_lookup(domain)
    } else {
        None
    };
    if cached_dns_ip.is_some() {
        agent::trace_fetch_cache_hit(b"dns", domain);
    }
    if can_reuse_openai {
        unsafe {
            let mut k = 0usize;
            while k < domain_len {
                FETCH_DOMAIN[k] = domain[k];
                k += 1;
            }
            fetch_set_domain_len(domain_len);
            k = 0;
            while k < path_len {
                FETCH_PATH[k] = path[k];
                k += 1;
            }
            fetch_set_path_len(path_len);
            FETCH_SRC_IP = src_ip_value;
            FETCH_REPLY_IP = reply_ip_value;
            FETCH_SRC_PORT = src_port_value;
            fetch_set_https(https);
            FETCH_DEBUG_SET_HTTPS_BYTE = fetch_https_byte_raw();
            FETCH_PROXY = true;
            FETCH_TARGET_PORT = target_port_value;
            FETCH_DST_PORT = PROXY_PORT;
            FETCH_HAVE_FIXED_IP = false;
            FETCH_OPENAI_REQUEST = openai_request;
            FETCH_OPENAI_REUSABLE = false;
            FETCH_PEER_CLOSED = false;
            FETCH_RETRY = 0;
            FETCH_NEXT_MS = 0;
            FETCH_GOT_RESP = false;
            FETCH_TX_USED = net::tx_used_idx();
            FETCH_TX_INFLIGHT = false;
            FETCH_HTTP_SENT = false;
            FETCH_HTTP_RETRY = 0;
            FETCH_HTTP_SEQ = 0;
            FETCH_HTTP_LEN = 0;
            FETCH_ACK_SENT = true;
            FETCH_DEADLINE_MS = 0;
            fetch_set_rounds(0);
            FETCH_REPLY_SENT = false;
            FETCH_REPLY_PENDING = false;
            FETCH_REPLY_BYTES = 0;
            FETCH_CHUNK_IDX = 0;
            FETCH_DONE_PRINTED = false;
            if !FETCH_REDIRECT_START {
                FETCH_REDIRECTS = 0;
            }
            FETCH_REDIRECT_PENDING = false;
            FETCH_SUPPRESS_OK = false;
            FETCH_TRACE_LAST_STATE = 0xff;
            clear_fetch_error_reason();
            http_reset();
            TLS_HTTP_LEN = 0;
            TLS_HTTP_OFF = 0;
            TLS_TCP_LOGS = 0;
            TLS_CERT_LOGS = 0;
            FETCH_STATE = FETCH_TLS_HTTP;
        }
        fetch_trace_phase_if_needed();
        return true;
    }
    unsafe {
        FETCH_DEBUG_START_CALLS = FETCH_DEBUG_START_CALLS.wrapping_add(1);
        let mut k = 0usize;
        while k < domain_len {
            FETCH_DOMAIN[k] = domain[k];
            k += 1;
        }
        fetch_set_domain_len(domain_len);
        k = 0;
        while k < path_len {
            FETCH_PATH[k] = path[k];
            k += 1;
        }
        fetch_set_path_len(path_len);
        FETCH_SRC_IP = src_ip_value;
        FETCH_REPLY_IP = reply_ip_value;
        FETCH_SRC_PORT = src_port_value;
        FETCH_TCP_SRC_PORT = NEXT_TCP_PORT;
        NEXT_TCP_PORT = NEXT_TCP_PORT.wrapping_add(1);
        if NEXT_TCP_PORT == 0 {
            NEXT_TCP_PORT = 40000;
        }
        fetch_set_https(https);
        FETCH_DEBUG_SET_HTTPS = fetch_https_raw();
        FETCH_DEBUG_SET_HTTPS_BYTE = fetch_https_byte_raw();
        FETCH_DEBUG_SET_PATH_LEN = fetch_path_len_raw();
        FETCH_SOCKS_SENT = false;
        FETCH_PROXY = use_proxy;
        FETCH_TARGET_PORT = target_port_value;
        FETCH_DST_PORT = if FETCH_PROXY { PROXY_PORT } else { FETCH_TARGET_PORT };
        FETCH_HAVE_FIXED_IP = fixed_ip_present;
        FETCH_FIXED_IP = fixed_ip_value;
        FETCH_OPENAI_REQUEST = openai_request;
        FETCH_OPENAI_REUSABLE = false;
        FETCH_HAVE_GW = have_gw;
        if have_gw {
            FETCH_GW_MAC = gw_mac;
        }
        FETCH_HAVE_DNS = have_dns;
        if have_dns {
            FETCH_DNS_MAC = dns_mac;
        }
        FETCH_RETRY = 0;
        FETCH_NEXT_MS = 0;
        if DEBUG_NET {
            uart::write_str("fetch after-next round=");
            uart::write_u64_dec(FETCH_ROUNDS as u64);
            uart::write_str("/");
            uart::write_u64_dec(fetch_rounds_raw() as u64);
            uart::write_str(" next_ms=");
            uart::write_u64_dec(FETCH_NEXT_MS);
            uart::write_str("/");
            uart::write_u64_dec(fetch_next_ms_raw());
            uart::write_str("\n");
        }
        FETCH_GOT_RESP = false;
        FETCH_SEQ = 0x1000;
        FETCH_ACK = 0;
        FETCH_TCP_ESTABLISHED = false;
        FETCH_TX_USED = net::tx_used_idx();
        if DEBUG_NET {
            uart::write_str("fetch after-txused round=");
            uart::write_u64_dec(FETCH_ROUNDS as u64);
            uart::write_str("/");
            uart::write_u64_dec(fetch_rounds_raw() as u64);
            uart::write_str(" next_ms=");
            uart::write_u64_dec(FETCH_NEXT_MS);
            uart::write_str("/");
            uart::write_u64_dec(fetch_next_ms_raw());
            uart::write_str("\n");
        }
        FETCH_TX_INFLIGHT = false;
        FETCH_HTTP_SENT = false;
        FETCH_HTTP_RETRY = 0;
        FETCH_HTTP_SEQ = 0;
        FETCH_HTTP_LEN = 0;
        FETCH_ACK_SENT = false;
        FETCH_DEADLINE_MS = 0;
        fetch_set_rounds(0);
        if DEBUG_NET {
            uart::write_str("fetch after-round round=");
            uart::write_u64_dec(FETCH_ROUNDS as u64);
            uart::write_str("/");
            uart::write_u64_dec(fetch_rounds_raw() as u64);
            uart::write_str(" next_ms=");
            uart::write_u64_dec(FETCH_NEXT_MS);
            uart::write_str("/");
            uart::write_u64_dec(fetch_next_ms_raw());
            uart::write_str("\n");
        }
        FETCH_REPLY_SENT = false;
        FETCH_REPLY_PENDING = false;
        FETCH_REPLY_BYTES = 0;
        FETCH_CHUNK_IDX = 0;
        FETCH_DONE_PRINTED = false;
        FETCH_PEER_CLOSED = false;
        if !FETCH_REDIRECT_START {
            FETCH_REDIRECTS = 0;
        }
        FETCH_REDIRECT_PENDING = false;
        FETCH_SUPPRESS_OK = false;
        FETCH_TRACE_LAST_STATE = 0xff;
        if DEBUG_NET {
            uart::write_str("fetch pre-reset state=");
            uart::write_bytes(fetch_state_name(FETCH_STATE));
            uart::write_str("/");
            uart::write_bytes(fetch_state_name(fetch_state_raw()));
            uart::write_str(" round=");
            uart::write_u64_dec(FETCH_ROUNDS as u64);
            uart::write_str("/");
            uart::write_u64_dec(fetch_rounds_raw() as u64);
            uart::write_str(" next_ms=");
            uart::write_u64_dec(FETCH_NEXT_MS);
            uart::write_str("/");
            uart::write_u64_dec(fetch_next_ms_raw());
            uart::write_str("\n");
        }
        clear_fetch_error_reason();
        http_reset();
        FETCH_DEBUG_POST_RESET_HTTPS = fetch_https_raw();
        FETCH_DEBUG_POST_RESET_HTTPS_BYTE = fetch_https_byte_raw();
        FETCH_DEBUG_POST_RESET_PATH_LEN = fetch_path_len_raw();
        if DEBUG_NET {
            uart::write_str("fetch post-reset state=");
            uart::write_bytes(fetch_state_name(FETCH_STATE));
            uart::write_str("/");
            uart::write_bytes(fetch_state_name(fetch_state_raw()));
            uart::write_str(" round=");
            uart::write_u64_dec(FETCH_ROUNDS as u64);
            uart::write_str("/");
            uart::write_u64_dec(fetch_rounds_raw() as u64);
            uart::write_str(" next_ms=");
            uart::write_u64_dec(FETCH_NEXT_MS);
            uart::write_str("/");
            uart::write_u64_dec(fetch_next_ms_raw());
            uart::write_str("\n");
        }
        let now_ms = timer::ticks_to_ms(timer::counter_ticks(), timer::counter_freq_hz());
        if DEBUG_NET {
            uart::write_str("fetch now_ms=");
            uart::write_u64_dec(now_ms);
            uart::write_str(" cooldown=");
            uart::write_u64_dec(FETCH_TRANSPORT_COOLDOWN_UNTIL_MS);
            uart::write_str("\n");
        }
        if FETCH_TRANSPORT_COOLDOWN_UNTIL_MS > now_ms {
            FETCH_NEXT_MS = FETCH_TRANSPORT_COOLDOWN_UNTIL_MS;
        }
        if src_port_value == 0 {
            FETCH_HAVE_PEER = false;
        }
        TLS_HTTP_LEN = 0;
        TLS_HTTP_OFF = 0;
        TLS_TCP_LOGS = 0;
        TLS_CERT_LOGS = 0;
        if FETCH_HAVE_FIXED_IP && !FETCH_PROXY {
            FETCH_DST_IP = FETCH_FIXED_IP;
            FETCH_STATE = FETCH_SYN;
        } else if AUTO_USE_FIXED_IP && !FETCH_PROXY {
            FETCH_DST_IP = AUTO_FIXED_IP;
            FETCH_STATE = FETCH_SYN;
        } else if let Some(ip) = cached_dns_ip {
            FETCH_DST_IP = ip;
            FETCH_STATE = if FETCH_HAVE_GW { FETCH_SYN } else { FETCH_ARP };
        } else if FETCH_PROXY {
            FETCH_DST_IP = PROXY_IP;
            FETCH_STATE = FETCH_SYN;
        } else {
            FETCH_STATE = if FETCH_HAVE_GW { FETCH_DNS } else { FETCH_ARP };
        }
        FETCH_DEBUG_POST_STATE_HTTPS = fetch_https_raw();
        FETCH_DEBUG_POST_STATE_HTTPS_BYTE = fetch_https_byte_raw();
        FETCH_DEBUG_POST_STATE_PATH_LEN = fetch_path_len_raw();
        fetch_set_https(https);
        FETCH_DEBUG_FINAL_HTTPS = fetch_https_raw();
        FETCH_DEBUG_FINAL_HTTPS_BYTE = fetch_https_byte_raw();
        if DEBUG_NET {
            uart::write_str("fetch post-state state=");
            uart::write_bytes(fetch_state_name(FETCH_STATE));
            uart::write_str("/");
            uart::write_bytes(fetch_state_name(fetch_state_raw()));
            uart::write_str(" round=");
            uart::write_u64_dec(FETCH_ROUNDS as u64);
            uart::write_str("/");
            uart::write_u64_dec(fetch_rounds_raw() as u64);
            uart::write_str(" next_ms=");
            uart::write_u64_dec(FETCH_NEXT_MS);
            uart::write_str("/");
            uart::write_u64_dec(fetch_next_ms_raw());
            uart::write_str("\n");
        }
    }
    if DEBUG_NET {
        uart::write_str("fetch start\n");
        uart::write_str("fetch init state=");
        uart::write_bytes(fetch_state_name(unsafe { FETCH_STATE }));
        uart::write_str("/");
        uart::write_bytes(fetch_state_name(fetch_state_raw()));
        uart::write_str(" round=");
        uart::write_u64_dec(unsafe { FETCH_ROUNDS as u64 });
        uart::write_str("/");
        uart::write_u64_dec(fetch_rounds_raw() as u64);
        uart::write_str(" next_ms=");
        uart::write_u64_dec(unsafe { FETCH_NEXT_MS });
        uart::write_str("/");
        uart::write_u64_dec(fetch_next_ms_raw());
        uart::write_str("\n");
    }
    fetch_trace_phase_if_needed();
    true
}

fn fetch_start(
    domain: &[u8],
    path: &[u8],
    src_ip: [u8; 4],
    reply_ip: [u8; 4],
    src_port: u16,
    https: bool,
) -> bool {
    fetch_start_ex(
        domain,
        path,
        src_ip,
        reply_ip,
        src_port,
        if https { 1 } else { 0 },
        if https { 443 } else { 80 },
        None,
    )
}

fn start_m5_bridge_fetch(path: &[u8]) -> bool {
    unsafe {
        FETCH_METHOD_POST = false;
        FETCH_BODY_LEN = 0;
        FETCH_EXTRA_HEADER_LEN = 0;
        FETCH_OAUTH_ACTIVE = false;
    }
    fetch_start_ex(
        M5_BRIDGE_DOMAIN,
        path,
        [10, 0, 2, 15],
        [0, 0, 0, 0],
        0,
        0,
        M5_BRIDGE_PORT,
        Some(M5_BRIDGE_IP),
    )
}

fn start_m5_bridge_post(path: &[u8], body: &[u8]) -> bool {
    if body.len() > unsafe { FETCH_BODY.len() } {
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
    start_m5_bridge_post_current_body(path, body.len())
}

fn start_m5_bridge_post_current_body(path: &[u8], body_len: usize) -> bool {
    if body_len > unsafe { FETCH_BODY.len() } {
        return false;
    }
    unsafe {
        FETCH_METHOD_POST = true;
        FETCH_BODY_LEN = body_len;
        FETCH_EXTRA_HEADER_LEN = 0;
        FETCH_OAUTH_ACTIVE = false;
    }
    fetch_start_ex(
        M5_BRIDGE_DOMAIN,
        path,
        [10, 0, 2, 15],
        [0, 0, 0, 0],
        0,
        0,
        M5_BRIDGE_PORT,
        Some(M5_BRIDGE_IP),
    )
}

fn start_tls_local_fetch() -> bool {
    unsafe {
        FETCH_METHOD_POST = false;
        FETCH_BODY_LEN = 0;
        FETCH_EXTRA_HEADER_LEN = 0;
        FETCH_OAUTH_ACTIVE = false;
    }
    fetch_start_ex(
        TLS_LOCAL_DOMAIN,
        TLS_LOCAL_PATH,
        [10, 0, 2, 15],
        [0, 0, 0, 0],
        0,
        1,
        TLS_LOCAL_PORT,
        Some(TLS_LOCAL_IP),
    )
}

fn start_m5_bridge_openai_post_current_body(body_len: usize) -> bool {
    start_m5_bridge_post_current_body(M5_BRIDGE_OPENAI_RESPONSES_PATH, body_len)
}

fn arp_mac_for(ip: [u8; 4]) -> Option<[u8; 6]> {
    unsafe {
        if FETCH_HAVE_PEER && ip == FETCH_REPLY_IP {
            return Some(FETCH_PEER_MAC);
        }
    }
    if let Some(mac) = net::lookup_arp_peer(ip) {
        return Some(mac);
    }
    unsafe {
        if ip == [10, 0, 2, 2] && FETCH_HAVE_GW {
            return Some(FETCH_GW_MAC);
        }
        if ip == [10, 0, 2, 3] && FETCH_HAVE_DNS {
            return Some(FETCH_DNS_MAC);
        }
    }
    None
}

fn fetch_tick(nb: usize, mac: [u8; 6]) {
    let now = timer::ticks_to_ms(timer::counter_ticks(), timer::counter_freq_hz());
    let state = unsafe { FETCH_STATE };
    if state == FETCH_IDLE {
        return;
    }
    unsafe {
        if FETCH_DEADLINE_MS == 0 {
            FETCH_DEADLINE_MS = now + 30_000;
        } else if now > FETCH_DEADLINE_MS {
            if DEBUG_NET {
                if debug_output_enabled() {
                    uart::write_str("fetch timeout\n");
                }
            }
            if FETCH_ROUNDS < FETCH_MAX_ROUNDS {
                fetch_set_rounds(FETCH_ROUNDS.wrapping_add(1));
                FETCH_RETRY = 0;
                FETCH_NEXT_MS = 0;
                FETCH_HTTP_SENT = false;
                FETCH_HTTP_RETRY = 0;
                FETCH_ACK_SENT = false;
                FETCH_GOT_RESP = false;
                FETCH_SOCKS_SENT = false;
                FETCH_DST_IP = if FETCH_PROXY { PROXY_IP } else { [0, 0, 0, 0] };
                FETCH_STATE = if FETCH_PROXY { FETCH_SYN } else if FETCH_HAVE_GW { FETCH_DNS } else { FETCH_ARP };
                FETCH_DEADLINE_MS = now + 30_000;
                return;
            }
            set_fetch_error_reason_if_empty(b"network request timed out");
            FETCH_STATE = FETCH_DONE;
            return;
        }
    }
    if state != FETCH_ARP && unsafe { !FETCH_HAVE_GW } {
        unsafe { FETCH_STATE = FETCH_ARP; }
        return;
    }
    let tx_used = net::tx_used_idx();
    if unsafe { FETCH_TX_INFLIGHT } {
        if tx_used != unsafe { FETCH_TX_USED } {
            unsafe {
                FETCH_TX_USED = tx_used;
                FETCH_TX_INFLIGHT = false;
            }
        } else {
            return;
        }
    }
    let gw_ip = [10, 0, 2, 2];
    let src_ip = unsafe { FETCH_SRC_IP };
    let domain_len = fetch_domain_len_raw();
    let path_len = fetch_path_len_raw();
    if domain_len > unsafe { FETCH_DOMAIN.len() } || path_len > unsafe { FETCH_PATH.len() } {
        uart::write_str("fetch len corrupt domain_len=");
        uart::write_u64_dec(domain_len as u64);
        uart::write_str(" path_len=");
        uart::write_u64_dec(path_len as u64);
        uart::write_str("\n");
        unsafe {
            FETCH_STATE = FETCH_DONE;
        }
        return;
    }
    let domain = unsafe { &FETCH_DOMAIN[..domain_len] };
    let _path = unsafe { &FETCH_PATH[..path_len] };
    let peer_mac = unsafe { FETCH_GW_MAC };
    if state == FETCH_ARP {
        if DEBUG_NET {
            if debug_output_enabled() {
                uart::write_str("fetch arp tick\n");
            }
        }
        if unsafe { FETCH_HAVE_GW } {
            unsafe { FETCH_STATE = if FETCH_PROXY { FETCH_SYN } else { FETCH_DNS }; }
            return;
        }
        if now < unsafe { FETCH_NEXT_MS } {
            return;
        }
        if unsafe { FETCH_RETRY } >= FETCH_MAX_RETRY {
            unsafe {
                if FETCH_ROUNDS < FETCH_MAX_ROUNDS {
                    fetch_set_rounds(FETCH_ROUNDS.wrapping_add(1));
                    FETCH_RETRY = 0;
                    FETCH_NEXT_MS = 0;
                    FETCH_HTTP_SENT = false;
                    FETCH_HTTP_RETRY = 0;
                    FETCH_ACK_SENT = false;
                    FETCH_GOT_RESP = false;
                    FETCH_SOCKS_SENT = false;
                    FETCH_DST_IP = if FETCH_PROXY { PROXY_IP } else { [0, 0, 0, 0] };
                    FETCH_STATE = if FETCH_PROXY { FETCH_SYN } else if FETCH_HAVE_GW { FETCH_DNS } else { FETCH_ARP };
                    return;
                }
                set_fetch_error_reason_if_empty(b"gateway arp timed out");
                FETCH_STATE = FETCH_DONE;
            }
            return;
        }
        net::send_arp(nb, mac, src_ip, gw_ip);
        if DEBUG_NET {
            uart::write_str("arp gw send\n");
        }
        unsafe {
            FETCH_RETRY = FETCH_RETRY.wrapping_add(1);
            FETCH_NEXT_MS = now + 200;
            FETCH_TX_USED = tx_used;
            FETCH_TX_INFLIGHT = true;
        }
        return;
    }
    if state == FETCH_DNS {
        if DEBUG_NET {
            if debug_output_enabled() {
                uart::write_str("fetch dns tick\n");
            }
        }
        if now < unsafe { FETCH_NEXT_MS } {
            return;
        }
        if unsafe { FETCH_RETRY } >= FETCH_MAX_RETRY {
            unsafe {
                if FETCH_ROUNDS < FETCH_MAX_ROUNDS {
                    fetch_set_rounds(FETCH_ROUNDS.wrapping_add(1));
                    FETCH_RETRY = 0;
                    FETCH_NEXT_MS = 0;
                    FETCH_HTTP_SENT = false;
                    FETCH_HTTP_RETRY = 0;
                    FETCH_ACK_SENT = false;
                    FETCH_GOT_RESP = false;
                    FETCH_SOCKS_SENT = false;
                    FETCH_DST_IP = if FETCH_PROXY { PROXY_IP } else { [0, 0, 0, 0] };
                    FETCH_STATE = if FETCH_PROXY { FETCH_SYN } else if FETCH_HAVE_GW { FETCH_DNS } else { FETCH_ARP };
                    return;
                }
                set_fetch_error_reason_if_empty(b"dns lookup timed out");
                FETCH_STATE = FETCH_DONE;
            }
            return;
        }
        let dns_server = [10, 0, 2, 3];
        unsafe {
            if !FETCH_HAVE_DNS && FETCH_HAVE_GW {
                FETCH_DNS_MAC = FETCH_GW_MAC;
                FETCH_HAVE_DNS = true;
            }
        }
        if unsafe { !FETCH_HAVE_DNS } {
            if DEBUG_NET {
                uart::write_str("arp dns send\n");
            }
            net::send_arp(nb, mac, src_ip, dns_server);
            unsafe {
                FETCH_RETRY = FETCH_RETRY.wrapping_add(1);
                FETCH_NEXT_MS = now + 200;
                FETCH_TX_USED = tx_used;
                FETCH_TX_INFLIGHT = true;
            }
            return;
        }
        let dns_id = 0x1234u16;
        let dns_buf = unsafe { &mut DNS_BUF };
        let dns_len = dns_build_query(dns_buf, domain, dns_id);
        if dns_len == 0 {
            set_fetch_error_reason(b"dns request build failed");
            unsafe { FETCH_STATE = FETCH_DONE; }
            return;
        }
        if DEBUG_NET {
            uart::write_str("dns query send\n");
        }
        net::rx_rearm(nb);
        net::send_udp(nb, mac, src_ip, 53000, unsafe { FETCH_DNS_MAC }, dns_server, 53, &dns_buf[..dns_len]);
        unsafe {
            FETCH_RETRY = FETCH_RETRY.wrapping_add(1);
            FETCH_NEXT_MS = now + 2000;
            FETCH_TX_USED = tx_used;
            FETCH_TX_INFLIGHT = true;
        }
        return;
    }
    if state == FETCH_SYN {
        if DEBUG_NET {
            if debug_output_enabled() {
                uart::write_str("fetch syn tick\n");
            }
        }
        if now < unsafe { FETCH_NEXT_MS } {
            return;
        }
        if unsafe { FETCH_RETRY } >= FETCH_MAX_RETRY {
            unsafe {
                if FETCH_ROUNDS < FETCH_MAX_ROUNDS {
                    fetch_set_rounds(FETCH_ROUNDS.wrapping_add(1));
                    FETCH_RETRY = 0;
                    FETCH_NEXT_MS = 0;
                    FETCH_HTTP_SENT = false;
                    FETCH_HTTP_RETRY = 0;
                    FETCH_ACK_SENT = false;
                    FETCH_GOT_RESP = false;
                    FETCH_SOCKS_SENT = false;
                    FETCH_DST_IP = if FETCH_PROXY { PROXY_IP } else { [0, 0, 0, 0] };
                    FETCH_STATE = if FETCH_PROXY { FETCH_SYN } else if FETCH_HAVE_GW { FETCH_DNS } else { FETCH_ARP };
                    return;
                }
                set_fetch_error_reason_if_empty(b"tcp connect timed out");
                FETCH_STATE = FETCH_DONE;
            }
            return;
        }
        let dst_ip = unsafe { FETCH_DST_IP };
        let src_port = unsafe { FETCH_TCP_SRC_PORT };
        let dst_port = unsafe { FETCH_DST_PORT };
        let seq = unsafe { FETCH_SEQ };
        net::rx_rearm(nb);
        net::send_tcp(nb, mac, src_ip, src_port, peer_mac, dst_ip, dst_port, seq, 0, 0x02, &[]);
        unsafe {
            FETCH_RETRY = FETCH_RETRY.wrapping_add(1);
            FETCH_NEXT_MS = now + 2000;
            FETCH_TX_USED = tx_used;
            FETCH_TX_INFLIGHT = true;
        }
        return;
    }
    if state == FETCH_SOCKS_HELLO {
        if now < unsafe { FETCH_NEXT_MS } {
            return;
        }
        if unsafe { FETCH_RETRY } >= FETCH_MAX_RETRY {
            set_fetch_error_reason_if_empty(b"proxy handshake timed out");
            unsafe { FETCH_STATE = FETCH_DONE; }
            return;
        }
        if unsafe { FETCH_TX_INFLIGHT } {
            return;
        }
        let dst_ip = unsafe { FETCH_DST_IP };
        let dst_port = unsafe { FETCH_DST_PORT };
        let seq = unsafe { FETCH_SEQ };
        let ack = unsafe { FETCH_ACK };
        let hello = [0x05u8, 0x01, 0x00];
        net::rx_rearm(nb);
        net::send_tcp(
            nb,
            mac,
            src_ip,
            unsafe { FETCH_TCP_SRC_PORT },
            unsafe { FETCH_GW_MAC },
            dst_ip,
            dst_port,
            seq,
            ack,
            0x18,
            &hello,
        );
        unsafe {
            FETCH_SEQ = FETCH_SEQ.wrapping_add(hello.len() as u32);
            FETCH_SOCKS_SENT = true;
            FETCH_RETRY = FETCH_RETRY.wrapping_add(1);
            FETCH_NEXT_MS = now + 2000;
            FETCH_TX_USED = tx_used;
            FETCH_TX_INFLIGHT = true;
        }
        return;
    }
    if state == FETCH_SOCKS_CONNECT {
        if now < unsafe { FETCH_NEXT_MS } {
            return;
        }
        if unsafe { FETCH_RETRY } >= FETCH_MAX_RETRY {
            set_fetch_error_reason_if_empty(b"proxy connect timed out");
            unsafe { FETCH_STATE = FETCH_DONE; }
            return;
        }
        if unsafe { FETCH_TX_INFLIGHT } {
            return;
        }
        let dst_ip = unsafe { FETCH_DST_IP };
        let dst_port = unsafe { FETCH_DST_PORT };
        let seq = unsafe { FETCH_SEQ };
        let ack = unsafe { FETCH_ACK };
        let domain_len = fetch_domain_len_raw();
        let domain = unsafe { &FETCH_DOMAIN[..domain_len] };
        if domain.len() > 255 {
            set_fetch_error_reason(b"proxy request host too long");
            unsafe { FETCH_STATE = FETCH_DONE; }
            return;
        }
        let mut req = [0u8; 300];
        let mut len = 0usize;
        req[len] = 0x05; len += 1;
        req[len] = 0x01; len += 1;
        req[len] = 0x00; len += 1;
        req[len] = 0x03; len += 1;
        req[len] = domain.len() as u8; len += 1;
        let mut j = 0usize;
        while j < domain.len() {
            req[len] = domain[j];
            len += 1;
            j += 1;
        }
        let port = unsafe { FETCH_TARGET_PORT };
        req[len] = (port >> 8) as u8; len += 1;
        req[len] = (port & 0xff) as u8; len += 1;
        net::rx_rearm(nb);
        net::send_tcp(
            nb,
            mac,
            src_ip,
            unsafe { FETCH_TCP_SRC_PORT },
            unsafe { FETCH_GW_MAC },
            dst_ip,
            dst_port,
            seq,
            ack,
            0x18,
            &req[..len],
        );
        unsafe {
            FETCH_SEQ = FETCH_SEQ.wrapping_add(len as u32);
            FETCH_SOCKS_SENT = true;
            FETCH_RETRY = FETCH_RETRY.wrapping_add(1);
            FETCH_NEXT_MS = now + 2000;
            FETCH_TX_USED = tx_used;
            FETCH_TX_INFLIGHT = true;
        }
        return;
    }
    if state == FETCH_HTTP {
        if unsafe { !FETCH_ACK_SENT } {
            if now < unsafe { FETCH_NEXT_MS } {
                return;
            }
            if unsafe { FETCH_TX_INFLIGHT } {
                return;
            }
            if DEBUG_NET {
                uart::write_str("tcp send ack\n");
            }
            net::send_tcp(
                nb,
                mac,
                src_ip,
                unsafe { FETCH_TCP_SRC_PORT },
                unsafe { FETCH_GW_MAC },
                unsafe { FETCH_DST_IP },
                    unsafe { FETCH_DST_PORT },
                unsafe { FETCH_SEQ },
                unsafe { FETCH_ACK },
                0x10,
                &[],
            );
            let tx_used = net::tx_used_idx();
            unsafe {
                FETCH_TX_USED = tx_used;
                FETCH_TX_INFLIGHT = true;
                FETCH_ACK_SENT = true;
                FETCH_NEXT_MS = now + 50;
            }
            return;
        }
        let sent = unsafe { FETCH_HTTP_SENT };
        if !sent {
            if now < unsafe { FETCH_NEXT_MS } {
                return;
            }
            if unsafe { FETCH_HTTP_RETRY } >= FETCH_MAX_RETRY {
                uart::write_str("http send retries exhausted\n");
                set_fetch_error_reason_if_empty(b"http send retries exhausted");
                unsafe { FETCH_STATE = FETCH_DONE; }
                return;
            }
            if unsafe { FETCH_TX_INFLIGHT } {
                return;
            }
            let dst_ip = unsafe { FETCH_DST_IP };
            let http_buf = unsafe { &mut HTTP_BUF };
            let domain_len = fetch_domain_len_raw();
            let path_len = fetch_path_len_raw();
            let domain = unsafe { &FETCH_DOMAIN[..domain_len] };
            let path = unsafe { &FETCH_PATH[..path_len] };
            let req_len = build_http_request(domain, path, http_buf);
            if req_len == 0 {
                set_fetch_error_reason(b"http request build failed");
                unsafe { FETCH_STATE = FETCH_DONE; }
                return;
            }
            if DEBUG_NET {
                uart::write_str("tcp send http\n");
            }
            let seq = unsafe { FETCH_SEQ };
            let ack = unsafe { FETCH_ACK };
            net::send_tcp(
                nb,
                mac,
                src_ip,
                unsafe { FETCH_TCP_SRC_PORT },
                unsafe { FETCH_GW_MAC },
                dst_ip,
                unsafe { FETCH_DST_PORT },
                seq,
                ack,
                0x18,
                &http_buf[..req_len],
            );
            let tx_used = net::tx_used_idx();
            unsafe {
                FETCH_HTTP_SEQ = seq;
                FETCH_HTTP_LEN = req_len as u16;
                FETCH_SEQ = FETCH_SEQ.wrapping_add(req_len as u32);
                FETCH_HTTP_SENT = true;
                FETCH_HTTP_RETRY = FETCH_HTTP_RETRY.wrapping_add(1);
                FETCH_TX_USED = tx_used;
                FETCH_TX_INFLIGHT = true;
                FETCH_NEXT_MS = now + 5000;
                FETCH_GOT_RESP = false;
            }
            return;
        }
        if debug_output_enabled() {
            uart::write_str("fetch http tick\n");
        }
        if now < unsafe { FETCH_NEXT_MS } {
            return;
        }
        if unsafe { FETCH_HTTP_RETRY } >= FETCH_MAX_RETRY {
            if debug_output_enabled() {
                uart::write_str("http timeout\n");
            }
            set_fetch_error_reason_if_empty(b"http response timed out");
            unsafe { FETCH_STATE = FETCH_DONE; }
            return;
        }
        if unsafe { FETCH_TX_INFLIGHT } {
            return;
        }
        if debug_output_enabled() {
            uart::write_str("http retry send\n");
        }
        let http_buf = unsafe { &mut HTTP_BUF };
        let domain_len = fetch_domain_len_raw();
        let path_len = fetch_path_len_raw();
        let domain = unsafe { &FETCH_DOMAIN[..domain_len] };
        let path = unsafe { &FETCH_PATH[..path_len] };
        let mut req_len = unsafe { FETCH_HTTP_LEN } as usize;
        if req_len == 0 {
            req_len = build_http_request(domain, path, http_buf);
            unsafe { FETCH_HTTP_LEN = req_len as u16; }
        }
        if req_len == 0 {
            set_fetch_error_reason(b"http retry request build failed");
            unsafe { FETCH_STATE = FETCH_DONE; }
            return;
        }
        let seq = unsafe { FETCH_HTTP_SEQ };
        let ack = unsafe { FETCH_ACK };
        net::send_tcp(
            nb,
            mac,
            src_ip,
            unsafe { FETCH_TCP_SRC_PORT },
            unsafe { FETCH_GW_MAC },
            unsafe { FETCH_DST_IP },
                unsafe { FETCH_DST_PORT },
            seq,
            ack,
            0x18,
            &http_buf[..req_len],
        );
        let tx_used = net::tx_used_idx();
        unsafe {
            FETCH_HTTP_RETRY = FETCH_HTTP_RETRY.wrapping_add(1);
            FETCH_TX_USED = tx_used;
            FETCH_TX_INFLIGHT = true;
            FETCH_NEXT_MS = now + 5000;
        }
        return;
    }
    if state == FETCH_TLS_HANDSHAKE {
        let ret = tls::handshake_step();
        if ret == 0 {
            trace_tls_export_if_any();
            trace_tls_cipher_diag_if_any();
            trace_tls_record_diag_if_any();
            trace_tls_cbc_diag_if_any();
            trace_tls_mac_diag_if_any();
            unsafe {
                FETCH_STATE = FETCH_TLS_HTTP;
                TLS_HTTP_OFF = 0;
                TLS_HTTP_LEN = 0;
            }
            return;
        }
        if tls::want_retry(ret) {
            return;
        }
        if DEBUG_NET {
            uart::write_str("tls handshake err: 0x");
            uart::write_u64_hex(ret as u64);
            uart::write_str("\n");
            let (x509_err, curve_id) = tls::debug_diag();
            let skx_err = tls::debug_skx_err();
            let skx_ret = tls::debug_skx_ret();
            if x509_err != 0 {
                uart::write_str("tls x509 err: 0x");
                uart::write_u64_hex(x509_err as u64);
                uart::write_str("\n");
            }
            if curve_id != 0 {
                uart::write_str("tls curve id: 0x");
                uart::write_u64_hex(curve_id as u64);
                uart::write_str("\n");
            }
            if skx_err != 0 {
                uart::write_str("tls skx err: 0x");
                uart::write_u64_hex(skx_err as u64);
                uart::write_str("\n");
            }
            if skx_ret != 0 {
                uart::write_str("tls skx ret: 0x");
                uart::write_u64_hex(skx_ret as u64);
                uart::write_str("\n");
            }
        }
        let (x509_err, curve_id) = tls::debug_diag();
        let skx_err = tls::debug_skx_err();
        let skx_ret = tls::debug_skx_ret();
        let verify_flags = tls::verify_result();
        let state = tls::state();
        trace_tls_export_if_any();
        trace_tls_cipher_diag_if_any();
        trace_tls_record_diag_if_any();
        trace_tls_cbc_diag_if_any();
        trace_tls_mac_diag_if_any();
        agent::trace_tls_last_tx(
            tls::last_tx_state(),
            tls::state_label(tls::last_tx_state()),
            tls::last_tx_out_ctr(),
            tls::last_tx_buf(),
        );
        agent::trace_tls_handshake_failure(
            ret,
            x509_err,
            curve_id,
            skx_err,
            skx_ret,
            verify_flags,
            state,
            tls::state_label(state),
            tls::error_label(ret),
        );
        set_fetch_error_reason(b"tls handshake failed");
        unsafe { FETCH_STATE = FETCH_DONE; }
        return;
    }
    if state == FETCH_TLS_HTTP {
        let http_buf = unsafe { &mut HTTP_BUF };
        let domain_len = fetch_domain_len_raw();
        let path_len = fetch_path_len_raw();
        let domain = unsafe { &FETCH_DOMAIN[..domain_len] };
        let path = unsafe { &FETCH_PATH[..path_len] };
        let mut req_len = unsafe { TLS_HTTP_LEN };
        if req_len == 0 {
            req_len = build_http_request(domain, path, http_buf);
            unsafe { TLS_HTTP_LEN = req_len; }
        }
        if req_len == 0 {
            set_fetch_error_reason(b"https request build failed");
            unsafe { FETCH_STATE = FETCH_DONE; }
            return;
        }
        let off = unsafe { TLS_HTTP_OFF };
        if off >= req_len {
            unsafe { FETCH_STATE = FETCH_TLS_READ; }
            unsafe { FETCH_NEXT_MS = now + 5000; }
            return;
        }
        let ret = tls::write_step(&http_buf[off..req_len]);
        if ret > 0 {
            unsafe { TLS_HTTP_OFF = off + ret as usize; }
            return;
        }
        if tls::want_retry(ret) {
            return;
        }
        let state = tls::state();
        agent::trace_tls_io_failure(
            b"https_request",
            ret,
            tls::verify_result(),
            tls::check_pending(),
            state,
            tls::state_label(state),
            tls::error_label(ret),
        );
        uart::write_str("tls write err: 0x");
        uart::write_u64_hex(ret as u64);
        uart::write_str("\n");
        set_fetch_error_reason(b"tls write failed");
        unsafe { FETCH_STATE = FETCH_DONE; }
        return;
    }
    if state == FETCH_TLS_READ {
        if fetch_http_response_complete() {
            unsafe { FETCH_STATE = FETCH_DONE; }
            return;
        }
        if now > unsafe { FETCH_NEXT_MS } && unsafe { FETCH_GOT_RESP } {
            unsafe { FETCH_STATE = FETCH_DONE; }
            return;
        }
        let http_buf = unsafe { &mut HTTP_BUF };
        let ret = tls::read_step(http_buf);
        if ret > 0 {
            let n = if ret as usize > http_buf.len() { http_buf.len() } else { ret as usize };
            http_feed(nb, mac, &http_buf[..n]);
            unsafe {
                FETCH_GOT_RESP = true;
                FETCH_NEXT_MS = now + 3000;
                if !FETCH_REPLY_SENT && !FETCH_SUPPRESS_OK && FETCH_SRC_PORT != 0 {
                    FETCH_REPLY_PENDING = true;
                }
            }
            return;
        }
        if ret == 0 || tls::is_peer_close(ret) {
            unsafe {
                FETCH_PEER_CLOSED = true;
            }
            unsafe { FETCH_STATE = FETCH_DONE; }
            return;
        }
        if tls::want_retry(ret) {
            return;
        }
        let state = tls::state();
        agent::trace_tls_io_failure(
            b"https_read",
            ret,
            tls::verify_result(),
            tls::check_pending(),
            state,
            tls::state_label(state),
            tls::error_label(ret),
        );
        uart::write_str("tls read err: 0x");
        uart::write_u64_hex(ret as u64);
        uart::write_str("\n");
        set_fetch_error_reason(b"tls read failed");
        unsafe { FETCH_STATE = FETCH_DONE; }
        return;
    }
}

fn reply_busy(
    nb: usize,
    mac: [u8; 6],
    peer_mac: [u8; 6],
    src_ip: [u8; 4],
    src_port: u16,
    reply_buf: &mut [u8],
    count: u64,
) {
    let mut out_len = udp_reply_prefix(reply_buf, count);
    let msg = b"busy";
    let mut j = 0usize;
    while j < msg.len() && out_len < reply_buf.len() {
        reply_buf[out_len] = msg[j];
        out_len += 1;
        j += 1;
    }
    net::send_udp(nb, mac, [10, 0, 2, 15], 5555, peer_mac, src_ip, src_port, &reply_buf[..out_len]);
}

fn send_http_chunks(
    nb: usize,
    mac: [u8; 6],
    src_ip: [u8; 4],
    src_port: u16,
    data: &[u8],
) {
    if src_port == 0 {
        return;
    }
    let peer_mac = match arp_mac_for(src_ip) {
        Some(m) => m,
        None => return,
    };
    let reply_buf = unsafe { &mut UDP_REPLY_BUF };
    let mut offset = 0usize;
    while offset < data.len() {
        let remaining = unsafe { FETCH_MAX_REPLY_BYTES.saturating_sub(FETCH_REPLY_BYTES) };
        if remaining == 0 {
            return;
        }
        let mut chunk_len = data.len() - offset;
        if chunk_len > remaining {
            chunk_len = remaining;
        }
        if chunk_len > FETCH_CHUNK_BYTES {
            chunk_len = FETCH_CHUNK_BYTES;
        }
        let mut out_len = udp_reply_prefix(reply_buf, unsafe { FETCH_CHUNK_IDX } as u64);
        let prefix = b"http chunk ";
        let mut j = 0usize;
        while j < prefix.len() && out_len < reply_buf.len() {
            reply_buf[out_len] = prefix[j];
            out_len += 1;
            j += 1;
        }
        let mut idx = unsafe { FETCH_CHUNK_IDX };
        let mut digits = [0u8; 5];
        let mut n = 0usize;
        if idx == 0 {
            digits[0] = b'0';
            n = 1;
        } else {
            while idx > 0 && n < digits.len() {
                digits[n] = b'0' + (idx % 10) as u8;
                idx /= 10;
                n += 1;
            }
        }
        let mut k = 0usize;
        while k < n && out_len < reply_buf.len() {
            reply_buf[out_len] = digits[n - 1 - k];
            out_len += 1;
            k += 1;
        }
        if out_len < reply_buf.len() {
            reply_buf[out_len] = b' ';
            out_len += 1;
        }
        let mut m = 0usize;
        while m < chunk_len && out_len < reply_buf.len() {
            reply_buf[out_len] = data[offset + m];
            out_len += 1;
            m += 1;
        }
        let sent1 = net::send_udp(
            nb,
            mac,
            [10, 0, 2, 15],
            5555,
            peer_mac,
            src_ip,
            src_port,
            &reply_buf[..out_len],
        );
        if sent1 {
            let _ = net::send_udp(
                nb,
                mac,
                [10, 0, 2, 15],
                5555,
                peer_mac,
                src_ip,
                src_port,
                &reply_buf[..out_len],
            );
            unsafe {
                FETCH_REPLY_BYTES = FETCH_REPLY_BYTES.saturating_add(chunk_len);
                FETCH_CHUNK_IDX = FETCH_CHUNK_IDX.wrapping_add(1);
            }
        } else {
            return;
        }
        offset += chunk_len;
    }
}

fn uart_prompt() {
    uart_end_input_color();
    clear_inline_status();
    unsafe {
        UART_PROMPT_COUNT = UART_PROMPT_COUNT.wrapping_add(1);
        UART_INPUT_ESCAPE_ACTIVE = false;
    }
    uart::write_str("Goal > ");
}

fn trace_output_enabled() -> bool {
    unsafe { UI_TRACE_ENABLED }
}

fn debug_output_enabled() -> bool {
    unsafe { UI_DEBUG_ENABLED }
}

fn clear_fetch_error_reason() {
    unsafe {
        FETCH_ERROR_REASON_LEN = 0;
    }
}

fn fetch_reason_is_transport_retryable(reason: &[u8]) -> bool {
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

fn set_fetch_error_reason(reason: &[u8]) {
    unsafe {
        let mut n = reason.len();
        if n > FETCH_ERROR_REASON.len() {
            n = FETCH_ERROR_REASON.len();
        }
        let mut i = 0usize;
        while i < n {
            FETCH_ERROR_REASON[i] = reason[i];
            i += 1;
        }
        FETCH_ERROR_REASON_LEN = n;
        if fetch_reason_is_transport_retryable(&FETCH_ERROR_REASON[..FETCH_ERROR_REASON_LEN]) {
            let now = timer::ticks_to_ms(timer::counter_ticks(), timer::counter_freq_hz());
            let until = now.saturating_add(FETCH_TRANSPORT_COOLDOWN_MS);
            if until > FETCH_TRANSPORT_COOLDOWN_UNTIL_MS {
                FETCH_TRANSPORT_COOLDOWN_UNTIL_MS = until;
            }
        }
    }
}

fn set_fetch_error_reason_if_empty(reason: &[u8]) {
    if fetch_error_reason().is_empty() {
        set_fetch_error_reason(reason);
    }
}

fn fetch_error_reason() -> &'static [u8] {
    unsafe { &FETCH_ERROR_REASON[..FETCH_ERROR_REASON_LEN] }
}

fn fetch_http_response_complete() -> bool {
    unsafe { HTTP_PARSE_STATE == 5 }
}

fn fetch_openai_reuse_candidate(domain: &[u8], path: &[u8], https: bool, target_port: u16) -> bool {
    if !NATIVE_OPENAI_REUSE {
        return false;
    }
    if !crate::openai::is_responses_target(domain, path, https, target_port) {
        return false;
    }
    unsafe {
        FETCH_OPENAI_REUSABLE
            && FETCH_TCP_ESTABLISHED
            && fetch_https_raw()
            && FETCH_PROXY
            && !FETCH_PEER_CLOSED
            && FETCH_DST_IP == PROXY_IP
            && FETCH_DST_PORT == PROXY_PORT
            && FETCH_TARGET_PORT == 443
    }
}

fn fetch_can_keep_openai_transport_open() -> bool {
    if !NATIVE_OPENAI_REUSE {
        return false;
    }
    unsafe {
        FETCH_OPENAI_REQUEST
            && FETCH_TCP_ESTABLISHED
            && !FETCH_PEER_CLOSED
            && !HTTP_IS_CHUNKED
            && HTTP_CONTENT_LEN != 0
            && fetch_http_response_complete()
            && !tls::check_pending()
    }
}

fn fetch_current_close_seq_ack() -> (u32, u32) {
    unsafe {
        if fetch_https_raw() && FETCH_TCP_ESTABLISHED {
            (tls::current_seq(), tls::current_ack())
        } else {
            (FETCH_SEQ, FETCH_ACK)
        }
    }
}

fn fetch_try_tls_close_notify() {
    unsafe {
        if !fetch_https_raw() || !FETCH_TCP_ESTABLISHED {
            return;
        }
    }
    let pending = tls::check_pending();
    let verify_flags = tls::verify_result();
    let ret = tls::close_notify_step();
    agent::trace_tls_close_notify(ret, verify_flags, pending, tls::error_label(ret));
}

fn print_tls_status() {
    let state = tls::state();
    uart::write_str("tls state=");
    uart::write_bytes(tls::state_label(state));
    uart::write_str(" verify=");
    uart::write_u64_dec(tls::verify_result() as u64);
    uart::write_str(" pending=");
    if tls::check_pending() {
        uart::write_str("true");
    } else {
        uart::write_str("false");
    }
    uart::write_str("\n");
    uart::write_str("tls export_count=");
    uart::write_u64_dec(tls::export_count() as u64);
    uart::write_str(" in_ctr=");
    uart::write_u64_dec(tls::in_ctr());
    uart::write_str(" out_ctr=");
    uart::write_u64_dec(tls::cur_out_ctr());
    uart::write_str("\n");
    uart::write_str("tls aes_self=");
    uart::write_u64_dec(tls::aes256_zero_key_self_hash());
    uart::write_str(" key_hash=");
    uart::write_u64_dec(tls::export_client_write_key_hash());
    uart::write_str(" key_prefix=");
    uart::write_u64_dec(tls::export_client_write_key_prefix());
    uart::write_str("\n");
    uart::write_str("tls key_aes_hash=");
    uart::write_u64_dec(tls::export_client_write_key_aes_zero_hash());
    uart::write_str(" key_aes_hash_static=");
    uart::write_u64_dec(tls::export_client_write_key_aes_zero_hash_static());
    uart::write_str("\n");
}

fn print_fetch_status() {
    uart::write_str("fetch state=");
    uart::write_bytes(fetch_state_name(unsafe { FETCH_STATE }));
    uart::write_str("/");
    uart::write_bytes(fetch_state_name(fetch_state_raw()));
    uart::write_str(" https=");
    uart::write_str(if fetch_https_raw() { "true" } else { "false" });
    uart::write_str(" proxy=");
    uart::write_str(if unsafe { FETCH_PROXY } { "true" } else { "false" });
    uart::write_str(" inflight=");
    uart::write_str(if unsafe { FETCH_TX_INFLIGHT } { "true" } else { "false" });
    uart::write_str("\n");
    uart::write_str("fetch retry=");
    uart::write_u64_dec(unsafe { FETCH_RETRY as u64 });
    uart::write_str(" round=");
    uart::write_u64_dec(unsafe { FETCH_ROUNDS as u64 });
    uart::write_str("/");
    uart::write_u64_dec(fetch_rounds_raw() as u64);
    uart::write_str(" next_ms=");
    uart::write_u64_dec(unsafe { FETCH_NEXT_MS });
    uart::write_str("/");
    uart::write_u64_dec(fetch_next_ms_raw());
    uart::write_str(" deadline_ms=");
    uart::write_u64_dec(unsafe { FETCH_DEADLINE_MS });
    uart::write_str("\n");
    uart::write_str("fetch seq=");
    uart::write_u64_dec(unsafe { FETCH_SEQ as u64 });
    uart::write_str(" ack=");
    uart::write_u64_dec(unsafe { FETCH_ACK as u64 });
    uart::write_str(" src_port=");
    uart::write_u64_dec(unsafe { FETCH_TCP_SRC_PORT as u64 });
    uart::write_str(" dst_port=");
    uart::write_u64_dec(unsafe { FETCH_DST_PORT as u64 });
    uart::write_str("\n");
    uart::write_str("fetch domain=");
    let domain_len = fetch_domain_len_raw();
    if domain_len <= unsafe { FETCH_DOMAIN.len() } {
        uart::write_bytes(unsafe { &FETCH_DOMAIN[..domain_len] });
    } else {
        uart::write_str("<corrupt>");
    }
    uart::write_str(" path=");
    let path_len = fetch_path_len_raw();
    if path_len <= unsafe { FETCH_PATH.len() } {
        uart::write_bytes(unsafe { &FETCH_PATH[..path_len] });
    } else {
        uart::write_str("<corrupt>");
    }
    uart::write_str("\n");
    uart::write_str("fetch lens domain_len=");
    uart::write_u64_dec(domain_len as u64);
    uart::write_str(" path_len=");
    uart::write_u64_dec(path_len as u64);
    uart::write_str("\n");
    let reason = fetch_error_reason();
    uart::write_str("fetch reason=");
    if reason.is_empty() {
        uart::write_str("<empty>");
    } else {
        uart::write_bytes(reason);
    }
    uart::write_str("\n");
    uart::write_str("fetch dbg calls=");
    uart::write_u64_dec(unsafe { FETCH_DEBUG_START_CALLS as u64 });
    uart::write_str(" set_https=");
    uart::write_str(if unsafe { FETCH_DEBUG_SET_HTTPS } { "true" } else { "false" });
    uart::write_str("/");
    uart::write_u64_dec(unsafe { FETCH_DEBUG_SET_HTTPS_BYTE as u64 });
    uart::write_str(" set_path_len=");
    uart::write_u64_dec(unsafe { FETCH_DEBUG_SET_PATH_LEN as u64 });
    uart::write_str("\n");
    uart::write_str("fetch dbg post_reset_https=");
    uart::write_str(if unsafe { FETCH_DEBUG_POST_RESET_HTTPS } { "true" } else { "false" });
    uart::write_str("/");
    uart::write_u64_dec(unsafe { FETCH_DEBUG_POST_RESET_HTTPS_BYTE as u64 });
    uart::write_str(" post_reset_path_len=");
    uart::write_u64_dec(unsafe { FETCH_DEBUG_POST_RESET_PATH_LEN as u64 });
    uart::write_str("\n");
    uart::write_str("fetch dbg post_state_https=");
    uart::write_str(if unsafe { FETCH_DEBUG_POST_STATE_HTTPS } { "true" } else { "false" });
    uart::write_str("/");
    uart::write_u64_dec(unsafe { FETCH_DEBUG_POST_STATE_HTTPS_BYTE as u64 });
    uart::write_str(" post_state_path_len=");
    uart::write_u64_dec(unsafe { FETCH_DEBUG_POST_STATE_PATH_LEN as u64 });
    uart::write_str("\n");
    uart::write_str("fetch dbg synack_https=");
    uart::write_str(if unsafe { FETCH_DEBUG_SYNACK_HTTPS } { "true" } else { "false" });
    uart::write_str(" synack_path_len=");
    uart::write_u64_dec(unsafe { FETCH_DEBUG_SYNACK_PATH_LEN as u64 });
    uart::write_str("\n");
    uart::write_str("fetch dbg final_https=");
    uart::write_str(if unsafe { FETCH_DEBUG_FINAL_HTTPS } { "true" } else { "false" });
    uart::write_str("/");
    uart::write_u64_dec(unsafe { FETCH_DEBUG_FINAL_HTTPS_BYTE as u64 });
    uart::write_str(" last_rounds_set=");
    uart::write_u64_dec(unsafe { FETCH_DEBUG_LAST_ROUNDS_SET as u64 });
    uart::write_str("\n");
    uart::write_str("fetch raw agent_mode=");
    uart::write_u64_hex(agent_mode_raw() as u64);
    uart::write_str(" agent_phase=");
    uart::write_u64_hex(agent_phase_raw() as u64);
    uart::write_str(" https_byte=");
    uart::write_u64_hex(fetch_https_byte_raw() as u64);
    uart::write_str(" proxy_byte=");
    uart::write_u64_hex(fetch_proxy_raw() as u64);
    uart::write_str(" retry_byte=");
    uart::write_u64_hex(fetch_retry_raw() as u64);
    uart::write_str(" state_byte=");
    uart::write_u64_hex(fetch_state_raw() as u64);
    uart::write_str("\n");
}

fn trace_tls_export_if_any() {
    let count = tls::export_count();
    if count == 0 {
        return;
    }
    agent::trace_tls_export(
        count,
        tls::export_client_random_prefix(),
        tls::export_server_random_prefix(),
        tls::export_client_random_hash(),
        tls::export_server_random_hash(),
        tls::export_master_hash(),
        tls::export_keyblock_hash(),
        tls::export_client_write_mac_hash(),
        tls::export_client_write_key_hash(),
        tls::export_client_write_key_prefix(),
        tls::export_server_write_key_hash(),
        tls::export_client_write_key_aes_zero_hash(),
        tls::export_client_write_key_aes_zero_hash_static(),
        tls::aes256_zero_key_self_hash(),
        tls::export_maclen(),
        tls::export_keylen(),
        tls::export_ivlen(),
        tls::export_prf_type(),
        tls::in_ctr(),
        tls::cur_out_ctr(),
        tls::has_transform_out(),
    );
}

fn trace_tls_cipher_diag_if_any() {
    let ciphersuite = tls::active_ciphersuite();
    if ciphersuite == 0 {
        return;
    }
    agent::trace_tls_cipher_diag(
        ciphersuite,
        tls::active_cipher_type(),
        tls::active_cipher_mode(),
        tls::active_cipher_operation(),
        tls::active_cipher_key_bitlen(),
        tls::active_iv_enc_prefix(),
        tls::active_iv_enc_hash(),
        tls::active_cipher_ctx_enc_aes_zero_hash(),
    );
}

fn trace_tls_record_diag_if_any() {
    agent::trace_tls_record_diag(
        tls::last_out_record_decrypt_ok(),
        tls::last_out_record_plaintext_hash(),
        tls::last_out_record_plaintext_len(),
        tls::last_out_record_padlen(),
    );
}

fn trace_tls_cbc_diag_if_any() {
    agent::trace_tls_cbc_diag(
        tls::last_cbc_reencrypt_match(),
        tls::last_cbc_plain_hash(),
        tls::last_cbc_expected_cipher_hash(),
        tls::last_cbc_actual_cipher_hash(),
        tls::last_cbc_len(),
    );
}

fn trace_tls_mac_diag_if_any() {
    agent::trace_tls_mac_diag(
        tls::last_mac_match(),
        tls::last_expected_mac_hash(),
        tls::last_actual_mac_hash(),
    );
}

fn fetch_close_current_transport() {
    unsafe {
        FETCH_OPENAI_REUSABLE = false;
        FETCH_OPENAI_REQUEST = false;
        if !FETCH_TCP_ESTABLISHED || FETCH_DST_IP == [0, 0, 0, 0] || FETCH_DST_PORT == 0 {
            tls::hard_reset();
            FETCH_TCP_ESTABLISHED = false;
            FETCH_PEER_CLOSED = false;
            return;
        }
        if NET_IFACE_READY {
            fetch_try_tls_close_notify();
            let (seq, ack) = fetch_current_close_seq_ack();
            net::send_tcp(
                NET_IFACE_NB,
                NET_IFACE_MAC,
                [10, 0, 2, 15],
                FETCH_TCP_SRC_PORT,
                FETCH_GW_MAC,
                FETCH_DST_IP,
                FETCH_DST_PORT,
                seq,
                ack,
                0x14,
                &[],
            );
        }
        tls::hard_reset();
        FETCH_TCP_ESTABLISHED = false;
        FETCH_PEER_CLOSED = false;
    }
}

pub(crate) fn fetch_finish_agent_idle() {
    unsafe {
        FETCH_EXTRA_HEADER_LEN = 0;
        FETCH_OAUTH_ACTIVE = false;
        FETCH_STATE = FETCH_IDLE;
        FETCH_TRACE_LAST_STATE = 0xff;
    }
    if unsafe { !FETCH_OPENAI_REUSABLE } {
        tls::hard_reset();
    }
}

pub(crate) fn fetch_prepare_openai_transport() {
    if unsafe { !FETCH_OPENAI_REUSABLE } {
        tls::hard_reset();
    }
}

fn fetch_best_effort_close(nb: usize, mac: [u8; 6]) {
    unsafe {
        if FETCH_PROXY || fetch_https_raw() {
            let now = timer::ticks_to_ms(timer::counter_ticks(), timer::counter_freq_hz());
            let until = now.saturating_add(FETCH_TRANSPORT_SUCCESS_COOLDOWN_MS);
            if until > FETCH_TRANSPORT_COOLDOWN_UNTIL_MS {
                FETCH_TRANSPORT_COOLDOWN_UNTIL_MS = until;
            }
        }
        if fetch_can_keep_openai_transport_open() {
            FETCH_OPENAI_REUSABLE = true;
            FETCH_PEER_CLOSED = false;
            return;
        }
        FETCH_OPENAI_REUSABLE = false;
        FETCH_OPENAI_REQUEST = false;
        if !FETCH_TCP_ESTABLISHED || FETCH_DST_IP == [0, 0, 0, 0] || FETCH_DST_PORT == 0 {
            tls::hard_reset();
            FETCH_TCP_ESTABLISHED = false;
            FETCH_PEER_CLOSED = false;
            return;
        }
        fetch_try_tls_close_notify();
        let (seq, ack) = fetch_current_close_seq_ack();
        net::send_tcp(
            nb,
            mac,
            [10, 0, 2, 15],
            FETCH_TCP_SRC_PORT,
            FETCH_GW_MAC,
            FETCH_DST_IP,
            FETCH_DST_PORT,
            seq,
            ack,
            0x14,
            &[],
        );
        tls::hard_reset();
        FETCH_TCP_ESTABLISHED = false;
        FETCH_PEER_CLOSED = false;
    }
}

fn fetch_state_name(state: u8) -> &'static [u8] {
    match state {
        FETCH_IDLE => b"idle",
        FETCH_ARP => b"arp",
        FETCH_DNS => b"dns",
        FETCH_SYN => b"tcp_connect",
        FETCH_HTTP => b"http_request",
        FETCH_TLS_HANDSHAKE => b"tls_handshake",
        FETCH_TLS_HTTP => b"https_request",
        FETCH_TLS_READ => b"https_read",
        FETCH_DONE => b"done",
        FETCH_SOCKS_HELLO => b"proxy_hello",
        FETCH_SOCKS_CONNECT => b"proxy_connect",
        _ => b"unknown",
    }
}

fn fetch_state_raw() -> u8 {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(FETCH_STATE)) }
}

fn fetch_https_raw() -> bool {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(FETCH_HTTPS)) }
}

fn fetch_set_https(value: bool) {
    unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!(FETCH_HTTPS), value) }
}

fn fetch_rounds_raw() -> u8 {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(FETCH_ROUNDS)) }
}

fn fetch_set_rounds(value: u8) {
    unsafe {
        FETCH_DEBUG_LAST_ROUNDS_SET = value;
        core::ptr::write_volatile(core::ptr::addr_of_mut!(FETCH_ROUNDS), value);
    }
}

fn fetch_next_ms_raw() -> u64 {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(FETCH_NEXT_MS)) }
}

fn fetch_domain_len_raw() -> usize {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(FETCH_DOMAIN_LEN)) }
}

fn fetch_set_domain_len(value: usize) {
    unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!(FETCH_DOMAIN_LEN), value) }
}

fn fetch_path_len_raw() -> usize {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(FETCH_PATH_LEN)) }
}

fn fetch_set_path_len(value: usize) {
    unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!(FETCH_PATH_LEN), value) }
}

fn agent_mode_raw() -> u8 {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(AGENT_MODE)) }
}

fn agent_phase_raw() -> u8 {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(AGENT_PHASE)) }
}

fn fetch_retry_raw() -> u8 {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(FETCH_RETRY)) }
}

fn fetch_proxy_raw() -> u8 {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(FETCH_PROXY)) as u8 }
}

fn fetch_https_byte_raw() -> u8 {
    unsafe { core::ptr::read_volatile(core::ptr::addr_of!(FETCH_HTTPS).cast::<u8>()) }
}

fn fetch_trace_phase_if_needed() {
    let state = unsafe { FETCH_STATE };
    if unsafe { FETCH_TRACE_LAST_STATE == state } {
        return;
    }
    unsafe {
        FETCH_TRACE_LAST_STATE = state;
    }
    if state == FETCH_IDLE {
        return;
    }
    agent::trace_fetch_phase_changed(
        fetch_state_name(state),
        unsafe { FETCH_RETRY },
        unsafe { FETCH_ROUNDS },
        unsafe { FETCH_PROXY },
    );
}

fn status_inline_enabled() -> bool {
    unsafe { UI_STATUS_INLINE }
}

fn clear_inline_status() {
    unsafe {
        if !UI_STATUS_ACTIVE {
            return;
        }
    }
    uart::write_str("\r\x1b[2K\r");
    unsafe {
        UI_STATUS_ACTIVE = false;
    }
}

fn show_inline_status(message: &[u8]) {
    clear_inline_status();
    uart::write_str("\r\x1b[2K[");
    uart::write_bytes(message);
    uart::write_str("]");
    unsafe {
        UI_STATUS_ACTIVE = true;
    }
}

fn uart_http_dump_enabled() -> bool {
    unsafe { FETCH_SRC_PORT == 0 && !AGENT_TASK_ACTIVE }
}

#[no_mangle]
pub extern "C" fn kmain(dtb_addr: usize) -> ! {
    uart::init();
    uart::set_silent(true);
    openai::init_embedded_api_key();
    let _dtb = dtb_addr;
    let base = 0x0a00_0000u64;
    let size = 0x200u64;
    if debug_output_enabled() {
        uart::write_str("hello world\n");
        uart::write_str("virtio-mmio base: 0x");
        uart::write_u64_hex(base);
        uart::write_str(" size: 0x");
        uart::write_u64_hex(size);
        uart::write_str("\n");
    }

    let mut found = false;
    let mut net_base: Option<usize> = None;
    for i in 0..32u64 {
        let b = base + i * size;
        if let Some(dev_id) = virtio::probe_mmio(b as usize) {
            if dev_id != 0 {
                if debug_output_enabled() {
                    uart::write_str("virtio dev @ 0x");
                    uart::write_u64_hex(b);
                    uart::write_str(" id: 0x");
                    uart::write_u64_hex(dev_id as u64);
                    uart::write_str(" ver: 0x");
                    let ver = unsafe { mmio::read32(b as usize + virtio::MMIO_VERSION) };
                    uart::write_u64_hex(ver as u64);
                    uart::write_str("\n");
                }
                found = true;
                if dev_id == virtio::VIRTIO_DEV_NET {
                    net_base = Some(b as usize);
                }
            }
        }
    }
    if !found && debug_output_enabled() {
        uart::write_str("virtio devices not found\n");
    }

    if let Some(nb) = net_base {
        net::reset_status(nb);
        net::set_status(nb, virtio::STATUS_ACK);
        net::dump_status(nb, "virtio-net");
        net::set_status(nb, virtio::STATUS_ACK | virtio::STATUS_DRIVER);
        net::dump_status(nb, "virtio-net");
        let ver = unsafe { mmio::read32(nb + virtio::MMIO_VERSION) };
        let modern = ver >= 2;
        let feats = virtio::read_device_features(nb, modern);
        let mut driver_feats = feats & net::VIRTIO_NET_F_MAC;
        if modern {
            driver_feats |= virtio::VIRTIO_F_VERSION_1;
        }
        virtio::write_driver_features(nb, driver_feats, modern);
        uart::write_str("virtio-net features: 0x");
        uart::write_u64_hex(feats);
        uart::write_str("\n");
        if modern {
            let want = virtio::STATUS_ACK | virtio::STATUS_DRIVER | virtio::STATUS_FEATURES_OK;
            net::set_status(nb, want);
            let got = unsafe { mmio::read32(nb + virtio::MMIO_STATUS) };
            uart::write_str("virtio-net features_ok status: 0x");
            uart::write_u64_hex(got as u64);
            uart::write_str("\n");
        }
        uart::write_str("rxq addr: 0x");
        uart::write_u64_hex(core::ptr::addr_of!(net::RXQ) as u64);
        uart::write_str("\n");
        uart::write_str("rxbuf addr: 0x");
        uart::write_u64_hex(net::rx_buf_addr() as u64);
        uart::write_str("\n");
        uart::write_str("txbuf addr: 0x");
        uart::write_u64_hex(core::ptr::addr_of!(net::TX_BUF) as u64);
        uart::write_str("\n");
        uart::write_str("txq addr: 0x");
        uart::write_u64_hex(core::ptr::addr_of!(net::TXQ) as u64);
        uart::write_str("\n");
        uart::write_str("virtq size: 0x");
        uart::write_u64_hex(core::mem::size_of::<net::Virtq>() as u64);
        uart::write_str("\n");
        uart::write_str("virtio-net queue init start\n");
        if net::init_queues(nb, modern) {
            uart::write_str("virtio-net queues ready\n");
            net::dump_queue(nb, 0, modern);
            net::dump_queue(nb, 1, modern);
            net::set_status(
                nb,
                virtio::STATUS_ACK | virtio::STATUS_DRIVER | virtio::STATUS_DRIVER_OK,
            );
            net::dump_status(nb, "virtio-net");
            net::dump_queue(nb, 0, modern);
            net::dump_queue(nb, 1, modern);
            let mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
            unsafe {
                NET_IFACE_READY = true;
                NET_IFACE_NB = nb;
                NET_IFACE_MAC = mac;
            }
            uart::write_str("virtio-net mac: ");
            for i in 0..6 {
                uart::write_u64_hex(mac[i] as u64);
                if i != 5 {
                    uart::write_str(":");
                }
            }
            uart::write_str("\n");
            let mut rx_last = net::rx_used_idx();
            uart::write_str("virtio-net rx buf addr: 0x");
            uart::write_u64_hex(net::rx_buf_addr() as u64);
            uart::write_str("\n");
            uart::write_str("virtio-net rx pre-read\n");
            if net::rx_buf_addr() == 0 {
                uart::write_str("virtio-net rx buf null\n");
            } else {
                let b = net::rx_buf_first_byte();
                uart::write_str("virtio-net rx read ok\n");
                uart::write_str("virtio-net rx buf first: 0x");
                uart::write_u64_hex(b as u64);
                uart::write_str("\n");
            }
            uart::write_str("virtio-net tx start\n");
            let mut last = net::tx_used_idx();
            if net::send_arp(nb, mac, [10, 0, 2, 15], [10, 0, 2, 2]) {
                uart::write_str("virtio-net tx wait\n");
                let mut attempt = 0u32;
                while attempt < 100 {
                    let cur = net::tx_used_idx();
                    if cur != last {
                        uart::write_str("virtio-net tx used idx: 0x");
                        uart::write_u64_hex(cur as u64);
                        uart::write_str("\n");
                        break;
                    }
                    if attempt == 0 {
                        uart::write_str("virtio-net tx used idx: 0x");
                        uart::write_u64_hex(cur as u64);
                        uart::write_str("\n");
                    }
                    timer::delay_ms(10);
                    attempt += 1;
                    last = cur;
                }
                if attempt >= 100 {
                    uart::write_str("virtio-net tx timeout\n");
                }
            }
            uart::write_str("virtio-net rx wait\n");
            let mut rx_attempt = 0u32;
            while rx_attempt < 100 {
                let cur = net::rx_used_idx();
                if cur != rx_last {
                    uart::write_str("virtio-net rx used idx: 0x");
                    uart::write_u64_hex(cur as u64);
                    uart::write_str("\n");
                    uart::write_str("virtio-net rx detail start\n");
                    if let Some((id, len)) = net::rx_used_elem_fields(cur) {
                        let _ = net::rx_set_current(id, len);
                        uart::write_str("virtio-net rx used id: 0x");
                        uart::write_u64_hex(id as u64);
                        uart::write_str(" len: 0x");
                        uart::write_u64_hex(len as u64);
                        uart::write_str("\n");
                        if net::parse_rx_arp() {
                            if let Some((peer_mac, peer_ip)) = net::last_arp_peer() {
                                uart::write_str("virtio-net rx rearm\n");
                                net::rx_rearm(nb);
                                uart::write_str("virtio-net tx icmp echo\n");
                                net::send_icmp_echo(nb, mac, [10, 0, 2, 15], peer_mac, peer_ip);
                                let mut rx2_attempt = 0u32;
                                let mut rx2_last = cur;
                                while rx2_attempt < 100 {
                                    let cur2 = net::rx_used_idx();
                                    if cur2 != rx2_last {
                                        uart::write_str("virtio-net rx used idx: 0x");
                                        uart::write_u64_hex(cur2 as u64);
                                        uart::write_str("\n");
                                        if let Some((id2, len2)) = net::rx_used_elem_fields(cur2) {
                                            let _ = net::rx_set_current(id2, len2);
                                        }
                                        if !net::parse_rx_icmp_reply() {
                                            uart::write_str("virtio-net rx icmp parse failed\n");
                                        }
                                        break;
                                    }
                                    timer::delay_ms(10);
                                    rx2_attempt += 1;
                                    rx2_last = cur2;
                                }
                            }
                        } else if !net::parse_rx_icmp_reply() {
                            uart::write_str("virtio-net rx parse skipped/failed\n");
                        }
                    } else {
                        uart::write_str("virtio-net rx used elem none\n");
                    }
                    uart::write_str("virtio-net rx detail end\n");
                    break;
                }
                if rx_attempt == 0 {
                    uart::write_str("virtio-net rx used idx: 0x");
                    uart::write_u64_hex(cur as u64);
                    uart::write_str("\n");
                }
                timer::delay_ms(10);
                rx_attempt += 1;
                rx_last = cur;
            }
            if debug_output_enabled() {
                uart::write_str("virtio-net mac read deferred\n");
                uart::write_str("virtio-net udp listen: 5555\n");
            }
            net::rx_rearm(nb);
            let mut last_rx = net::rx_used_idx();
            let mut udp_count: u64 = 0;
            uart::set_silent(false);
            if unsafe { !UART_PROMPT } {
                uart_prompt();
                unsafe { UART_PROMPT = true; }
            }
            if AUTO_TLS_LOCAL_FETCH {
                let _ = start_tls_local_fetch();
            } else if AUTO_FETCH {
                let _ = fetch_start(AUTO_DOMAIN, AUTO_PATH, [10, 0, 2, 15], [0, 0, 0, 0], 0, false);
            }
            loop {
                let mut cur = net::rx_used_idx();
                'rx: while cur != last_rx {
                    last_rx = last_rx.wrapping_add(1);
                    if let Some((id, len)) = net::rx_used_elem_fields(last_rx) {
                        let _ = net::rx_set_current(id, len);
                    }
                    let arp_needed = unsafe { FETCH_STATE == FETCH_ARP };
                    let arp_seen = if arp_needed && net::rx_eth_type() == 0x0806 {
                        net::parse_rx_arp()
                    } else {
                        false
                    };
                    if arp_seen {
                        if let Some(m) = net::lookup_arp_peer([10, 0, 2, 2]) {
                            unsafe {
                                FETCH_GW_MAC = m;
                                FETCH_HAVE_GW = true;
                                if FETCH_STATE == FETCH_ARP {
                                    FETCH_STATE = if FETCH_PROXY { FETCH_SYN } else if FETCH_DST_IP != [0, 0, 0, 0] { FETCH_SYN } else { FETCH_DNS };
                                    FETCH_RETRY = 0;
                                    FETCH_NEXT_MS = 0;
                                }
                            }
                            if DEBUG_NET {
                                uart::write_str("arp gw learned\n");
                            }
                        }
                        if let Some(m) = net::lookup_arp_peer([10, 0, 2, 3]) {
                            unsafe {
                                FETCH_DNS_MAC = m;
                                FETCH_HAVE_DNS = true;
                            }
                            if DEBUG_NET {
                                uart::write_str("arp dns learned\n");
                            }
                        }
                    }
                    let udp_info = net::parse_rx_udp_any();
                    let tcp_info = net::parse_rx_tcp();

                    let mut dns_consumed = false;
                    if unsafe { FETCH_STATE } == FETCH_DNS {
                        if let Some((src_ip2, src_port, dst_port, payload_addr, payload_len)) = udp_info {
                            if src_port == 53 && dst_port == 53000 {
                                let dns_buf = unsafe { &mut DNS_BUF };
                                let n = net::rx_copy(payload_addr, payload_len, dns_buf);
                                if let Some(ip) = dns_parse_response(dns_buf, n, 0x1234) {
                                    let domain_len = fetch_domain_len_raw();
                                    let domain = unsafe { &FETCH_DOMAIN[..domain_len] };
                                    dns_cache_store(domain, ip);
                                    unsafe {
                                        FETCH_DST_IP = ip;
                                        FETCH_RETRY = 0;
                                        FETCH_NEXT_MS = 0;
                                        FETCH_STATE = FETCH_SYN;
                                    }
                                    if DEBUG_NET {
                                        uart::write_str("dns ok\n");
                                        uart::write_str("dns ip: ");
                                        uart::write_u64_hex(ip[0] as u64);
                                        uart::write_str(".");
                                        uart::write_u64_hex(ip[1] as u64);
                                        uart::write_str(".");
                                        uart::write_u64_hex(ip[2] as u64);
                                        uart::write_str(".");
                                        uart::write_u64_hex(ip[3] as u64);
                                        uart::write_str("\n");
                                    }
                                } else {
                                    uart::write_str("dns parse fail\n");
                                }
                                dns_consumed = true;
                            }
                        } else {
                            let eth = net::rx_eth_type();
                            if DEBUG_NET {
                                uart::write_str("dns rx eth=0x");
                                uart::write_u64_hex(eth as u64);
                                if let Some(proto) = net::rx_ip_proto() {
                                    uart::write_str(" ip proto=0x");
                                    uart::write_u64_hex(proto as u64);
                                }
                                if let Some(ip) = net::rx_ip_src() {
                                    uart::write_str(" src=");
                                    uart::write_u64_hex(ip[0] as u64);
                                    uart::write_str(".");
                                    uart::write_u64_hex(ip[1] as u64);
                                    uart::write_str(".");
                                    uart::write_u64_hex(ip[2] as u64);
                                    uart::write_str(".");
                                    uart::write_u64_hex(ip[3] as u64);
                                }
                                uart::write_str("\n");
                            }
                        }
                    } else if unsafe { FETCH_STATE } == FETCH_SYN {
                        if let Some((src_ip2, s_port, d_port, s_seq, _s_ack, flags, _addr, _len)) = tcp_info {
                            if DEBUG_NET {
                                uart::write_str("tcp rx src=");
                                uart::write_u64_hex(src_ip2[0] as u64);
                                uart::write_str(".");
                                uart::write_u64_hex(src_ip2[1] as u64);
                                uart::write_str(".");
                                uart::write_u64_hex(src_ip2[2] as u64);
                                uart::write_str(".");
                                uart::write_u64_hex(src_ip2[3] as u64);
                                uart::write_str(" sport=");
                                uart::write_u64_hex(s_port as u64);
                                uart::write_str(" dport=");
                                uart::write_u64_hex(d_port as u64);
                                uart::write_str(" flags=0x");
                                uart::write_u64_hex(flags as u64);
                                uart::write_str("\n");
                            }
                            if src_ip2 == unsafe { FETCH_DST_IP } && s_port == unsafe { FETCH_DST_PORT } && d_port == unsafe { FETCH_TCP_SRC_PORT } {
                                if (flags & 0x12) == 0x12 {
                                    unsafe {
                                        FETCH_DEBUG_SYNACK_HTTPS = fetch_https_raw();
                                        FETCH_DEBUG_SYNACK_PATH_LEN = fetch_path_len_raw();
                                    }
                                    unsafe {
                                        FETCH_ACK = s_seq.wrapping_add(1);
                                        FETCH_SEQ = FETCH_SEQ.wrapping_add(1);
                                    }
                                    let tx_used = net::tx_used_idx();
                                    let now_ms = timer::ticks_to_ms(timer::counter_ticks(), timer::counter_freq_hz());
                                    if !(unsafe { FETCH_TX_INFLIGHT } && tx_used == unsafe { FETCH_TX_USED }) {
                                        if debug_output_enabled() {
                                            uart::write_str("tcp send ack\n");
                                        }
                                        net::send_tcp(
                                            nb,
                                            mac,
                                            [10, 0, 2, 15],
                                            unsafe { FETCH_TCP_SRC_PORT },
                                            unsafe { FETCH_GW_MAC },
                                            unsafe { FETCH_DST_IP },
            unsafe { FETCH_DST_PORT },
                                            unsafe { FETCH_SEQ },
                                            unsafe { FETCH_ACK },
                                            0x10,
                                            &[],
                                        );
                                        unsafe {
                                            FETCH_TX_USED = tx_used;
                                            FETCH_TX_INFLIGHT = true;
                                            FETCH_ACK_SENT = true;
                                            FETCH_TCP_ESTABLISHED = true;
                                            FETCH_HTTP_SENT = false;
                                            FETCH_HTTP_RETRY = 0;
                                            FETCH_GOT_RESP = false;
                                            FETCH_NEXT_MS = now_ms + 50;
                                        }
                                        if unsafe { FETCH_PROXY } {
                                            unsafe {
                                                FETCH_STATE = FETCH_SOCKS_HELLO;
                                                FETCH_RETRY = 0;
                                                FETCH_NEXT_MS = now_ms + 50;
                                                FETCH_SOCKS_SENT = false;
                                            }
                                        } else if fetch_https_raw() {
                                            let ok = tls::configure(
                                                nb,
                                                mac,
                                                [10, 0, 2, 15],
                                                unsafe { FETCH_DST_IP },
                                                unsafe { FETCH_GW_MAC },
                                                unsafe { FETCH_TCP_SRC_PORT },
                                                unsafe { FETCH_DST_PORT },
                                                unsafe { FETCH_SEQ },
                                                unsafe { FETCH_ACK },
                                                {
                                                    let domain_len = fetch_domain_len_raw();
                                                    unsafe { &FETCH_DOMAIN[..domain_len] }
                                                },
                                            );
                                            let tls_state = tls::state();
                                            agent::trace_tls_config(
                                                tls::last_reset_ret(),
                                                tls::last_hostname_ret(),
                                                tls_state,
                                                tls::state_label(tls_state),
                                                tls::in_ctr(),
                                                tls::cur_out_ctr(),
                                                tls::has_transform_out(),
                                                tls::aes256_zero_key_self_hash(),
                                            );
                                            if !ok {
                                                set_fetch_error_reason(b"tls configure failed");
                                            }
                                            unsafe { FETCH_STATE = if ok { FETCH_TLS_HANDSHAKE } else { FETCH_DONE }; }
                                        } else {
                                            unsafe { FETCH_STATE = FETCH_HTTP; }
                                        }
                                    } else {
                                        unsafe {
                                            FETCH_ACK_SENT = false;
                                            FETCH_TCP_ESTABLISHED = true;
                                            FETCH_HTTP_SENT = false;
                                            FETCH_HTTP_RETRY = 0;
                                            FETCH_GOT_RESP = false;
                                            FETCH_NEXT_MS = now_ms + 50;
                                        }
                                        if unsafe { FETCH_PROXY } {
                                            unsafe {
                                                FETCH_STATE = FETCH_SOCKS_HELLO;
                                                FETCH_RETRY = 0;
                                                FETCH_NEXT_MS = now_ms + 50;
                                                FETCH_SOCKS_SENT = false;
                                            }
                                        } else if fetch_https_raw() {
                                            let ok = tls::configure(
                                                nb,
                                                mac,
                                                [10, 0, 2, 15],
                                                unsafe { FETCH_DST_IP },
                                                unsafe { FETCH_GW_MAC },
                                                unsafe { FETCH_TCP_SRC_PORT },
                                                unsafe { FETCH_DST_PORT },
                                                unsafe { FETCH_SEQ },
                                                unsafe { FETCH_ACK },
                                                {
                                                    let domain_len = fetch_domain_len_raw();
                                                    unsafe { &FETCH_DOMAIN[..domain_len] }
                                                },
                                            );
                                            let tls_state = tls::state();
                                            agent::trace_tls_config(
                                                tls::last_reset_ret(),
                                                tls::last_hostname_ret(),
                                                tls_state,
                                                tls::state_label(tls_state),
                                                tls::in_ctr(),
                                                tls::cur_out_ctr(),
                                                tls::has_transform_out(),
                                                tls::aes256_zero_key_self_hash(),
                                            );
                                            if !ok {
                                                set_fetch_error_reason(b"tls configure failed");
                                            }
                                            unsafe { FETCH_STATE = if ok { FETCH_TLS_HANDSHAKE } else { FETCH_DONE }; }
                                        } else {
                                            unsafe { FETCH_STATE = FETCH_HTTP; }
                                        }
                                    }
                                }
                            }
                        }
                    } else if unsafe { FETCH_STATE } == FETCH_SOCKS_HELLO {
                        if let Some((src_ip2, s_port, d_port, s_seq, _s_ack, _flags, p_addr, p_len)) = tcp_info {
                            if src_ip2 == unsafe { FETCH_DST_IP } && s_port == unsafe { FETCH_DST_PORT } && d_port == unsafe { FETCH_TCP_SRC_PORT } {
                                if p_len >= 2 {
                                    let v = unsafe { mmio::read8(p_addr) };
                                    let m = unsafe { mmio::read8(p_addr + 1) };
                                    unsafe {
                                        FETCH_ACK = s_seq.wrapping_add(p_len as u32);
                                    }
                                        net::send_tcp(
                                            nb,
                                            mac,
                                            [10, 0, 2, 15],
                                            unsafe { FETCH_TCP_SRC_PORT },
                                            unsafe { FETCH_GW_MAC },
                                            unsafe { FETCH_DST_IP },
                                        unsafe { FETCH_DST_PORT },
                                        unsafe { FETCH_SEQ },
                                        unsafe { FETCH_ACK },
                                        0x10,
                                        &[],
                                    );
                                    if v == 0x05 && m == 0x00 {
                                        unsafe {
                                            FETCH_STATE = FETCH_SOCKS_CONNECT;
                                            FETCH_RETRY = 0;
                                            FETCH_NEXT_MS = 0;
                                            FETCH_SOCKS_SENT = false;
                                        }
                                    } else {
                                        set_fetch_error_reason(b"proxy handshake rejected");
                                        unsafe { FETCH_STATE = FETCH_DONE; }
                                    }
                                }
                            }
                        }
                    } else if unsafe { FETCH_STATE } == FETCH_SOCKS_CONNECT {
                        if let Some((src_ip2, s_port, d_port, s_seq, _s_ack, _flags, p_addr, p_len)) = tcp_info {
                            if src_ip2 == unsafe { FETCH_DST_IP } && s_port == unsafe { FETCH_DST_PORT } && d_port == unsafe { FETCH_TCP_SRC_PORT } {
                                if p_len >= 5 {
                                    let v = unsafe { mmio::read8(p_addr) };
                                    let r = unsafe { mmio::read8(p_addr + 1) };
                                    unsafe {
                                        FETCH_ACK = s_seq.wrapping_add(p_len as u32);
                                    }
                                    net::send_tcp(
                                        nb,
                                        mac,
                                        [10, 0, 2, 15],
                                        unsafe { FETCH_TCP_SRC_PORT },
                                        unsafe { FETCH_GW_MAC },
                                        unsafe { FETCH_DST_IP },
                                        unsafe { FETCH_DST_PORT },
                                        unsafe { FETCH_SEQ },
                                        unsafe { FETCH_ACK },
                                        0x10,
                                        &[],
                                    );
                                    if v == 0x05 && r == 0x00 {
                                        if fetch_https_raw() {
                                            let ok = tls::configure(
                                                nb,
                                                mac,
                                                [10, 0, 2, 15],
                                                unsafe { FETCH_DST_IP },
                                                unsafe { FETCH_GW_MAC },
                                                unsafe { FETCH_TCP_SRC_PORT },
                                                unsafe { FETCH_DST_PORT },
                                                unsafe { FETCH_SEQ },
                                                unsafe { FETCH_ACK },
                                                {
                                                    let domain_len = fetch_domain_len_raw();
                                                    unsafe { &FETCH_DOMAIN[..domain_len] }
                                                },
                                            );
                                            let tls_state = tls::state();
                                            agent::trace_tls_config(
                                                tls::last_reset_ret(),
                                                tls::last_hostname_ret(),
                                                tls_state,
                                                tls::state_label(tls_state),
                                                tls::in_ctr(),
                                                tls::cur_out_ctr(),
                                                tls::has_transform_out(),
                                                tls::aes256_zero_key_self_hash(),
                                            );
                                            if !ok {
                                                set_fetch_error_reason(b"tls configure failed");
                                            }
                                            unsafe { FETCH_STATE = if ok { FETCH_TLS_HANDSHAKE } else { FETCH_DONE }; }
                                        } else {
                                            unsafe { FETCH_STATE = FETCH_HTTP; }
                                        }
                                    } else {
                                        set_fetch_error_reason(b"proxy connect rejected");
                                        unsafe { FETCH_STATE = FETCH_DONE; }
                                    }
                                }
                            }
                        }
                    } else if unsafe { FETCH_STATE } == FETCH_HTTP {
                        if let Some((src_ip2, s_port, d_port, s_seq, s_ack, flags, p_addr, p_len)) = tcp_info {
                            if DEBUG_NET {
                                uart::write_str("tcp http rx src=");
                                uart::write_u64_hex(src_ip2[0] as u64);
                                uart::write_str(".");
                                uart::write_u64_hex(src_ip2[1] as u64);
                                uart::write_str(".");
                                uart::write_u64_hex(src_ip2[2] as u64);
                                uart::write_str(".");
                                uart::write_u64_hex(src_ip2[3] as u64);
                                uart::write_str(" sport=");
                                uart::write_u64_hex(s_port as u64);
                                uart::write_str(" dport=");
                                uart::write_u64_hex(d_port as u64);
                                uart::write_str(" len=");
                                uart::write_u64_hex(p_len as u64);
                                uart::write_str(" flags=0x");
                                uart::write_u64_hex(flags as u64);
                                uart::write_str(" ack=");
                                uart::write_u64_hex(s_ack as u64);
                                uart::write_str(" exp=");
                                uart::write_u64_hex(unsafe { FETCH_SEQ } as u64);
                                uart::write_str("\n");
                            }
                            if src_ip2 == unsafe { FETCH_DST_IP } && s_port == unsafe { FETCH_DST_PORT } && d_port == unsafe { FETCH_TCP_SRC_PORT } {
                                let expected_ack = unsafe { FETCH_ACK };
                                let mut new_payload_addr = p_addr;
                                let mut new_payload_len = p_len;
                                if p_len > 0 {
                                    let seq_cmp = tcp_seq_cmp(s_seq, expected_ack);
                                    if seq_cmp > 0 {
                                        new_payload_len = 0;
                                    } else if seq_cmp < 0 {
                                        let overlap = expected_ack.wrapping_sub(s_seq) as usize;
                                        if overlap >= p_len {
                                            new_payload_len = 0;
                                        } else {
                                            new_payload_addr += overlap;
                                            new_payload_len -= overlap;
                                        }
                                    }
                                    let mut offset = 0usize;
                                    while offset < new_payload_len {
                                        let mut buf = [0u8; FETCH_CHUNK_BYTES];
                                        let mut copy_len = new_payload_len - offset;
                                        if copy_len > buf.len() {
                                            copy_len = buf.len();
                                        }
                                        let mut k = 0usize;
                                        while k < copy_len {
                                            buf[k] = unsafe { mmio::read8(new_payload_addr + offset + k) };
                                            k += 1;
                                        }
                                        http_feed(nb, mac, &buf[..copy_len]);
                                        offset += copy_len;
                                    }
                                    if new_payload_len != 0 {
                                        unsafe {
                                            FETCH_ACK =
                                                s_seq.wrapping_add(p_len as u32);
                                        }
                                    }
                                    net::send_tcp(
                                        nb,
                                        mac,
                                        [10, 0, 2, 15],
                                        unsafe { FETCH_TCP_SRC_PORT },
                                        unsafe { FETCH_GW_MAC },
                                        unsafe { FETCH_DST_IP },
                                        unsafe { FETCH_DST_PORT },
                                        unsafe { FETCH_SEQ },
                                        unsafe { FETCH_ACK },
                                        0x10,
                                        &[],
                                    );
                                }
                                if (flags & 0x01) != 0 {
                                    unsafe {
                                        let fin_ack = s_seq.wrapping_add(p_len as u32).wrapping_add(1);
                                        if tcp_seq_cmp(fin_ack, FETCH_ACK) > 0 {
                                            FETCH_ACK = fin_ack;
                                        }
                                    }
                                    net::send_tcp(
                                        nb,
                                        mac,
                                        [10, 0, 2, 15],
                                        unsafe { FETCH_TCP_SRC_PORT },
                                        unsafe { FETCH_GW_MAC },
                                        unsafe { FETCH_DST_IP },
                                        unsafe { FETCH_DST_PORT },
                                        unsafe { FETCH_SEQ },
                                        unsafe { FETCH_ACK },
                                        0x10,
                                        &[],
                                    );
                                    unsafe { FETCH_STATE = FETCH_DONE; }
                                }
                            }
                        }
                    } else if matches!(unsafe { FETCH_STATE }, FETCH_TLS_HANDSHAKE | FETCH_TLS_HTTP | FETCH_TLS_READ) {
                        if let Some((src_ip2, s_port, d_port, s_seq, _s_ack, flags, p_addr, p_len)) = tcp_info {
                            if src_ip2 == unsafe { FETCH_DST_IP } && s_port == unsafe { FETCH_DST_PORT } && d_port == unsafe { FETCH_TCP_SRC_PORT } {
                                if DEBUG_NET && p_len == 0 && unsafe { TLS_TCP_LOGS } < 6 {
                                    uart::write_str("tls tcp flags=0x");
                                    uart::write_u64_hex(flags as u64);
                                    uart::write_str(" seq=");
                                    uart::write_u64_hex(s_seq as u64);
                                    uart::write_str("\n");
                                    unsafe { TLS_TCP_LOGS = TLS_TCP_LOGS.wrapping_add(1); }
                                }
                                let expected = tls::expected_ack();
                                let mut advanced = false;
                                if p_len > 0 {
                                    if unsafe { TLS_CERT_LOGS } < 3 && p_len >= 12 {
                                        let b0 = unsafe { mmio::read8(p_addr) };
                                        if b0 == 0x16 {
                                            let b5 = unsafe { mmio::read8(p_addr + 5) };
                                            if b5 == 0x0b {
                                                if DEBUG_NET {
                                                    uart::write_str("tls cert pkt=");
                                                    let mut k = 0usize;
                                                    while k < 12 {
                                                        let b = unsafe { mmio::read8(p_addr + k) };
                                                        uart::write_u64_hex(b as u64);
                                                        if k + 1 < 12 {
                                                            uart::write_str(" ");
                                                        }
                                                        k += 1;
                                                    }
                                                    uart::write_str("\n");
                                                    unsafe { TLS_CERT_LOGS = TLS_CERT_LOGS.wrapping_add(1); }
                                                }
                                            }
                                        }
                                    }
                                    advanced = tls::push_rx_payload_seq(s_seq, p_addr, p_len);
                                }
                                if (flags & 0x01) != 0 {
                                    let cur_ack = tls::expected_ack();
                                    if s_seq == cur_ack || (p_len > 0 && s_seq == expected) {
                                        tls::update_ack(cur_ack.wrapping_add(1));
                                        advanced = true;
                                    }
                                }
                                if p_len > 0 || (flags & 0x01) != 0 || advanced {
                                    tls::send_ack();
                                }
                            }
                        }
                    }

                    if !dns_consumed {
                        if let Some((src_ip, src_port, dst_port, payload_addr, payload_len)) = udp_info {
                        if dst_port == 5555 {
                            if let Some(peer_mac) = arp_mac_for(src_ip) {
                                let reply_buf = unsafe { &mut UDP_REPLY_BUF };
                                let payload = unsafe { &mut UDP_PAYLOAD_BUF };
                                let n = net::rx_copy(payload_addr, payload_len, payload);
                                let mut out_len = udp_reply_prefix(reply_buf, udp_count);
                                if starts_with(&payload[..], n, b"ping") {
                                    let pong = b"pong";
                                    let mut j = 0usize;
                                    while j < pong.len() && out_len < reply_buf.len() {
                                        reply_buf[out_len] = pong[j];
                                        out_len += 1;
                                        j += 1;
                                    }
                                } else if starts_with(&payload[..], n, b"mac") {
                                    let m = b"mac=";
                                    let mut j = 0usize;
                                    while j < m.len() && out_len < reply_buf.len() {
                                        reply_buf[out_len] = m[j];
                                        out_len += 1;
                                        j += 1;
                                    }
                                    let mut j = 0usize;
                                    while j < 6 && out_len + 2 < reply_buf.len() {
                                        let b = mac[j];
                                        let hi = b >> 4;
                                        let lo = b & 0x0f;
                                        reply_buf[out_len] = if hi < 10 { b'0' + hi } else { b'a' + (hi - 10) };
                                        reply_buf[out_len + 1] = if lo < 10 { b'0' + lo } else { b'a' + (lo - 10) };
                                        out_len += 2;
                                        if j != 5 && out_len < reply_buf.len() {
                                            reply_buf[out_len] = b':';
                                            out_len += 1;
                                        }
                                        j += 1;
                                    }
                                } else if starts_with(&payload[..], n, b"echo ") {
                                    let mut j = 5usize;
                                    while j < n && out_len < reply_buf.len() {
                                        reply_buf[out_len] = payload[j];
                                        out_len += 1;
                                        j += 1;
                                    }
                                } else if n == 4 && starts_with(&payload[..], n, b"sync") {
                                    if unsafe { FETCH_STATE } != FETCH_IDLE {
                                        let msg = b"busy";
                                        let mut j = 0usize;
                                        while j < msg.len() && out_len < reply_buf.len() {
                                            reply_buf[out_len] = msg[j];
                                            out_len += 1;
                                            j += 1;
                                        }
                                    } else {
                                        let msg = b"syncing";
                                        let mut j = 0usize;
                                        while j < msg.len() && out_len < reply_buf.len() {
                                            reply_buf[out_len] = msg[j];
                                            out_len += 1;
                                            j += 1;
                                        }
                                        net::send_udp(
                                            nb,
                                            mac,
                                            [10, 0, 2, 15],
                                            5555,
                                            peer_mac,
                                            src_ip,
                                            src_port,
                                            &reply_buf[..out_len],
                                        );
                                        udp_count = udp_count.wrapping_add(1);
                                        unsafe {
                                            FETCH_METHOD_POST = false;
                                            FETCH_BODY_LEN = 0;
                                            FETCH_EXTRA_HEADER_LEN = 0;
                                            FETCH_OAUTH_ACTIVE = false;
                                            FETCH_PEER_MAC = peer_mac;
                                            FETCH_HAVE_PEER = true;
                                        }
                                        let _ = fetch_start(SYNC_DOMAIN, SYNC_PATH, [10, 0, 2, 15], src_ip, src_port, true);
                                        if debug_output_enabled() {
                                    uart::write_str("virtio-net udp reply sent\n");
                                }
                                        net::rx_rearm(nb);
                                        cur = net::rx_used_idx();
                                        continue 'rx;
                                    }
                                } else if n > 5 && starts_with(&payload[..], n, b"time ") {
                                    if let Some(ts) = parse_u64(&payload[5..], n - 5) {
                                        set_oauth_time(ts);
                                        let msg = b"time set";
                                        let mut j = 0usize;
                                        while j < msg.len() && out_len < reply_buf.len() {
                                            reply_buf[out_len] = msg[j];
                                            out_len += 1;
                                            j += 1;
                                        }
                                    }
                                } else if (n > 6 && starts_with(&payload[..], n, b"tweet "))
                                    || (n > 11 && starts_with(&payload[..], n, b"post_tweet "))
                                {
                                    let mut start = if n > 11 && starts_with(&payload[..], n, b"post_tweet ") {
                                        11usize
                                    } else {
                                        6usize
                                    };
                                    while start < n && is_space(payload[start]) {
                                        start += 1;
                                    }
                                    let text = if start < n { &payload[start..n] } else { &[][..] };
                                    if unsafe { FETCH_STATE } != FETCH_IDLE {
                                        let msg = b"busy";
                                        let mut j = 0usize;
                                        while j < msg.len() && out_len < reply_buf.len() {
                                            reply_buf[out_len] = msg[j];
                                            out_len += 1;
                                            j += 1;
                                        }
                                    } else
                                    if prepare_tweet(text) {
                                        let msg = b"tweeting";
                                        let mut j = 0usize;
                                        while j < msg.len() && out_len < reply_buf.len() {
                                            reply_buf[out_len] = msg[j];
                                            out_len += 1;
                                            j += 1;
                                        }
                                        net::send_udp(
                                            nb,
                                            mac,
                                            [10, 0, 2, 15],
                                            5555,
                                            peer_mac,
                                            src_ip,
                                            src_port,
                                            &reply_buf[..out_len],
                                        );
                                        udp_count = udp_count.wrapping_add(1);
                                        unsafe {
                                            FETCH_PEER_MAC = peer_mac;
                                            FETCH_HAVE_PEER = true;
                                        }
                                        let _ = fetch_start(XAPI_DOMAIN, XAPI_PATH, [10, 0, 2, 15], src_ip, src_port, true);
                                        if debug_output_enabled() {
                                    uart::write_str("virtio-net udp reply sent\n");
                                }
                                        net::rx_rearm(nb);
                                        cur = net::rx_used_idx();
                                        continue 'rx;
                                    } else {
                                        let msg = b"tweet setup failed";
                                        let mut j = 0usize;
                                        while j < msg.len() && out_len < reply_buf.len() {
                                            reply_buf[out_len] = msg[j];
                                            out_len += 1;
                                            j += 1;
                                        }
                                    }
                                } else if n > 5 && starts_with(&payload[..], n, b"post ") {
                                    let mut start = 5usize;
                                    while start < n && is_space(payload[start]) {
                                        start += 1;
                                    }
                                    let mut end = start;
                                    while end < n && !is_space(payload[end]) {
                                        end += 1;
                                    }
                                    if start < end {
                                        let url_slice = &payload[start..end];
                                        if let Some(url) = parse_url(url_slice, url_slice.len()) {
                                            let mut body_start = end;
                                            while body_start < n && is_space(payload[body_start]) {
                                                body_start += 1;
                                            }
                                            let body = if body_start < n { &payload[body_start..n] } else { &[][..] };
                                            let domain = &url_slice[url.domain_start..url.domain_start + url.domain_len];
                                            let path = if url.path_len == 0 {
                                                &[][..]
                                            } else {
                                                &url_slice[url.path_start..url.path_start + url.path_len]
                                            };
                                            unsafe {
                                                FETCH_METHOD_POST = true;
                                                FETCH_EXTRA_HEADER_LEN = 0;
                                                FETCH_OAUTH_ACTIVE = false;
                                                let mut m = body.len();
                                                if m > FETCH_BODY.len() {
                                                    m = FETCH_BODY.len();
                                                }
                                                let mut i = 0usize;
                                                while i < m {
                                                    FETCH_BODY[i] = body[i];
                                                    i += 1;
                                                }
                                                FETCH_BODY_LEN = m;
                                            }
                                            let msg = b"fetching ";
                                            let mut j = 0usize;
                                            while j < msg.len() && out_len < reply_buf.len() {
                                                reply_buf[out_len] = msg[j];
                                                out_len += 1;
                                                j += 1;
                                            }
                                            let mut j = 0usize;
                                            while j < url.domain_len && out_len < reply_buf.len() {
                                                reply_buf[out_len] = url_slice[url.domain_start + j];
                                                out_len += 1;
                                                j += 1;
                                            }
                                            if url.path_len > 0 {
                                                j = 0;
                                                while j < url.path_len && out_len < reply_buf.len() {
                                                    reply_buf[out_len] = url_slice[url.path_start + j];
                                                    out_len += 1;
                                                    j += 1;
                                                }
                                            }
                                            net::send_udp(
                                                nb,
                                                mac,
                                                [10, 0, 2, 15],
                                                5555,
                                                peer_mac,
                                                src_ip,
                                                src_port,
                                                &reply_buf[..out_len],
                                            );
                                            udp_count = udp_count.wrapping_add(1);
                                            unsafe {
                                                FETCH_PEER_MAC = peer_mac;
                                                FETCH_HAVE_PEER = true;
                                            }
                                            let _ = fetch_start(domain, path, [10, 0, 2, 15], src_ip, src_port, url.https);
                                            if debug_output_enabled() {
                                    uart::write_str("virtio-net udp reply sent\n");
                                }
                                            net::rx_rearm(nb);
                                            cur = net::rx_used_idx();
                                            continue 'rx;
                                        }
                                    }
                                } else if let Some(url) = parse_url(&payload[..], n) {
                                    unsafe {
                                        FETCH_METHOD_POST = false;
                                        FETCH_BODY_LEN = 0;
                                        FETCH_EXTRA_HEADER_LEN = 0;
                                        FETCH_OAUTH_ACTIVE = false;
                                    }
                                    if unsafe { FETCH_STATE } != FETCH_IDLE {
                                        reply_busy(nb, mac, peer_mac, src_ip, src_port, reply_buf, udp_count);
                                        udp_count = udp_count.wrapping_add(1);
                                        if debug_output_enabled() {
                                    uart::write_str("virtio-net udp reply sent\n");
                                }
                                        net::rx_rearm(nb);
                                        cur = net::rx_used_idx();
                                        continue 'rx;
                                    }
                                    let msg = b"fetching ";
                                    let mut j = 0usize;
                                    while j < msg.len() && out_len < reply_buf.len() {
                                        reply_buf[out_len] = msg[j];
                                        out_len += 1;
                                        j += 1;
                                    }
                                    let mut j = 0usize;
                                    while j < url.domain_len && out_len < reply_buf.len() {
                                        reply_buf[out_len] = payload[url.domain_start + j];
                                        out_len += 1;
                                        j += 1;
                                    }
                                    if url.path_len > 0 {
                                        j = 0;
                                        while j < url.path_len && out_len < reply_buf.len() {
                                            reply_buf[out_len] = payload[url.path_start + j];
                                            out_len += 1;
                                            j += 1;
                                        }
                                    }
                                    net::send_udp(
                                        nb,
                                        mac,
                                        [10, 0, 2, 15],
                                        5555,
                                        peer_mac,
                                        src_ip,
                                        src_port,
                                        &reply_buf[..out_len],
                                    );
                                    udp_count = udp_count.wrapping_add(1);
                                    let domain = &payload[url.domain_start..url.domain_start + url.domain_len];
                                    let path = if url.path_len == 0 {
                                        &[][..]
                                    } else {
                                        &payload[url.path_start..url.path_start + url.path_len]
                                    };
                                    unsafe {
                                        FETCH_PEER_MAC = peer_mac;
                                        FETCH_HAVE_PEER = true;
                                    }
                                    let _ = fetch_start(domain, path, [10, 0, 2, 15], src_ip, src_port, url.https);
                                    if debug_output_enabled() {
                                    uart::write_str("virtio-net udp reply sent\n");
                                }
                                    net::rx_rearm(nb);
                                    cur = net::rx_used_idx();
                                    continue 'rx;
                                } else {
                                    let hello = b"Hello, I am WalleOS. ";
                                    let mut j = 0usize;
                                    while j < hello.len() && out_len < reply_buf.len() {
                                        reply_buf[out_len] = hello[j];
                                        out_len += 1;
                                        j += 1;
                                    }
                                    let mut j = 0usize;
                                    while j < n && out_len < reply_buf.len() {
                                        reply_buf[out_len] = payload[j];
                                        out_len += 1;
                                        j += 1;
                                    }
                                }
                                net::send_udp(
                                    nb,
                                    mac,
                                    [10, 0, 2, 15],
                                    5555,
                                    peer_mac,
                                    src_ip,
                                    src_port,
                                    &reply_buf[..out_len],
                                );
                                udp_count = udp_count.wrapping_add(1);
                                if debug_output_enabled() {
                                    uart::write_str("virtio-net udp reply sent\n");
                                }
                            }
                        }
                        }
                    }
                    net::rx_rearm(nb);
                    cur = net::rx_used_idx();
                }
                while let Some(b) = uart::read_byte() {
                    let skip_escape = unsafe { UART_INPUT_ESCAPE_ACTIVE };
                    if skip_escape {
                        if (0x40..=0x7e).contains(&b) {
                            unsafe {
                                UART_INPUT_ESCAPE_ACTIVE = false;
                            }
                        }
                        continue;
                    }
                    if b == 0x1b {
                        unsafe {
                            UART_INPUT_ESCAPE_ACTIVE = true;
                        }
                        continue;
                    }
                    if b == b'\r' || b == b'\n' {
                        uart_end_input_color();
                        uart::write_str("\n");
                        let line = unsafe { &UART_LINE_BUF[..UART_LINE_LEN] };
                        let len = unsafe { UART_LINE_LEN };
                        let prompt_count_before = unsafe { UART_PROMPT_COUNT };
                        handle_uart_line(line, len);
                        unsafe {
                            UART_LINE_LEN = 0;
                            UART_INPUT_ESCAPE_ACTIVE = false;
                        }
                        if unsafe {
                            !AGENT_TASK_ACTIVE
                                && FETCH_STATE == FETCH_IDLE
                                && UART_PROMPT_COUNT == prompt_count_before
                        } {
                            uart_prompt();
                        }
                    } else if b == 0x08 || b == 0x7f {
                        let cur = unsafe { UART_LINE_LEN };
                        if cur > 0 {
                            let next = unsafe { utf8_previous_boundary(&UART_LINE_BUF[..cur], cur) };
                            unsafe {
                                UART_LINE_LEN = next;
                            }
                            uart_redraw_input_line();
                        }
                    } else {
                        let cur = unsafe { UART_LINE_LEN };
                        if cur + 1 < UART_LINE_MAX {
                            unsafe { UART_LINE_BUF[cur] = b; UART_LINE_LEN = cur + 1; }
                            uart_begin_input_color();
                            uart::write_byte(b);
                        }
                    }
                }
                timer::delay_ms(10);
                fetch_trace_phase_if_needed();
                fetch_tick(nb, mac);
                fetch_trace_phase_if_needed();
                if unsafe { FETCH_REPLY_PENDING && !FETCH_REPLY_SENT } {
                    let reply_buf = unsafe { &mut UDP_REPLY_BUF };
                    let domain_len = fetch_domain_len_raw();
                    let domain = unsafe { &FETCH_DOMAIN[..domain_len] };
                    let src_ip = unsafe { FETCH_REPLY_IP };
                    let src_port = unsafe { FETCH_SRC_PORT };
                    if src_port != 0 {
                        if let Some(peer_mac) = arp_mac_for(src_ip) {
                            let mut out_len2 = udp_reply_prefix(reply_buf, udp_count);
                            let done: &[u8] = b"http ok ";
                            let mut k = 0usize;
                            while k < done.len() && out_len2 < reply_buf.len() {
                                reply_buf[out_len2] = done[k];
                                out_len2 += 1;
                                k += 1;
                            }
                            k = 0;
                            while k < domain.len() && out_len2 < reply_buf.len() {
                                reply_buf[out_len2] = domain[k];
                                out_len2 += 1;
                                k += 1;
                            }
                            let sent1 = net::send_udp(
                                nb,
                                mac,
                                [10, 0, 2, 15],
                                5555,
                                peer_mac,
                                src_ip,
                                src_port,
                                &reply_buf[..out_len2],
                            );
                            if sent1 {
                                let _ = net::send_udp(
                                    nb,
                                    mac,
                                    [10, 0, 2, 15],
                                    5555,
                                    peer_mac,
                                    src_ip,
                                    src_port,
                                    &reply_buf[..out_len2],
                                );
                                if debug_output_enabled() {
                                    uart::write_str("virtio-net udp reply sent\n");
                                }
                                udp_count = udp_count.wrapping_add(1);
                                unsafe {
                                    FETCH_REPLY_SENT = true;
                                    FETCH_REPLY_PENDING = false;
                                }
                            }
                        }
                    }
                }
                if unsafe { FETCH_STATE } == FETCH_DONE {
                    fetch_best_effort_close(nb, mac);
                    let reply_buf = unsafe { &mut UDP_REPLY_BUF };
                    let domain_len = fetch_domain_len_raw();
                    let domain = unsafe { &FETCH_DOMAIN[..domain_len] };
                    let src_ip = unsafe { FETCH_REPLY_IP };
                    let src_port = unsafe { FETCH_SRC_PORT };
                    let ok = unsafe { FETCH_GOT_RESP };
                    if agent::handle_m4_fetch_done(ok) {
                        continue;
                    }
                    if agent::handle_agent_fetch_done(ok) {
                        continue;
                    }
                    if unsafe { FETCH_REDIRECT_PENDING } {
                        let redir_domain = unsafe { &FETCH_REDIR_DOMAIN[..FETCH_REDIR_DOMAIN_LEN] };
                        let redir_path = unsafe { &FETCH_REDIR_PATH[..FETCH_REDIR_PATH_LEN] };
                        let redir_https = unsafe { FETCH_REDIR_HTTPS };
                        unsafe {
                            FETCH_METHOD_POST = false;
                            FETCH_BODY_LEN = 0;
                        }
                        unsafe { FETCH_REDIRECT_START = true; }
                        let started = fetch_start(
                            redir_domain,
                            redir_path,
                            [10, 0, 2, 15],
                            src_ip,
                            src_port,
                            redir_https,
                        );
                        unsafe {
                            FETCH_REDIRECT_START = false;
                            FETCH_REDIRECT_PENDING = false;
                            FETCH_SUPPRESS_OK = false;
                        }
                        if started {
                            continue;
                        }
                    }
                    let mut out_len2 = udp_reply_prefix(reply_buf, udp_count);
                    let done: &[u8] = if ok { b"http ok " } else { b"http fail " };
                    let mut j = 0usize;
                    while j < done.len() && out_len2 < reply_buf.len() {
                        reply_buf[out_len2] = done[j];
                        out_len2 += 1;
                        j += 1;
                    }
                    j = 0;
                    while j < domain.len() && out_len2 < reply_buf.len() {
                        reply_buf[out_len2] = domain[j];
                        out_len2 += 1;
                        j += 1;
                    }
                    if src_port != 0 && unsafe { !FETCH_SUPPRESS_OK } {
                        if unsafe { !FETCH_REPLY_SENT } {
                            if let Some(peer_mac) = arp_mac_for(src_ip) {
                                let sent1 = net::send_udp(
                                    nb,
                                    mac,
                                    [10, 0, 2, 15],
                                    5555,
                                    peer_mac,
                                    src_ip,
                                    src_port,
                                    &reply_buf[..out_len2],
                                );
                                if sent1 {
                                    let _ = net::send_udp(
                                        nb,
                                        mac,
                                        [10, 0, 2, 15],
                                        5555,
                                        peer_mac,
                                        src_ip,
                                        src_port,
                                        &reply_buf[..out_len2],
                                    );
                                    if debug_output_enabled() {
                                    uart::write_str("virtio-net udp reply sent\n");
                                }
                                } else {
                                    unsafe { FETCH_REPLY_PENDING = true; }
                                    continue;
                                }
                            }
                        }
                    } else {
                        if !unsafe { FETCH_DONE_PRINTED } {
                            uart::write_str("http ");
                            uart::write_str(if ok { "ok " } else { "fail " });
                            uart::write_bytes(domain);
                            uart::write_str("\n");
                            uart_prompt();
                            unsafe { FETCH_DONE_PRINTED = true; }
                        }
                        unsafe { FETCH_REPLY_PENDING = false; }
                    }
                    udp_count = udp_count.wrapping_add(1);
                    if unsafe { !FETCH_REPLY_PENDING } {
                        unsafe {
                            FETCH_EXTRA_HEADER_LEN = 0;
                            FETCH_OAUTH_ACTIVE = false;
                        }
                        unsafe { FETCH_STATE = FETCH_IDLE; }
                    }
                }
            }
        } else {
            uart::set_silent(false);
            uart::write_str("virtio-net queue init failed\n");
            let st = unsafe { mmio::read32(nb + virtio::MMIO_STATUS) };
            net::set_status(nb, st | virtio::STATUS_FAILED);
        }
    }
    uart::set_silent(false);
    loop {
        unsafe { core::arch::asm!("wfe"); }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    uart::write_str("panic");
    if let Some(loc) = info.location() {
        uart::write_str(" ");
        uart::write_str(loc.file());
        uart::write_str(":");
        uart::write_u64_dec(loc.line() as u64);
    }
    uart::write_str("\n");
    loop {
        unsafe { core::arch::asm!("wfe"); }
    }
}

#[no_mangle]
pub extern "C" fn exception_entry(vector: u64) -> ! {
    let esr: u64;
    let far: u64;
    let elr: u64;
    let spsr: u64;
    let current_el: u64;
    unsafe {
        core::arch::asm!("mrs {0}, esr_el1", out(reg) esr);
        core::arch::asm!("mrs {0}, far_el1", out(reg) far);
        core::arch::asm!("mrs {0}, elr_el1", out(reg) elr);
        core::arch::asm!("mrs {0}, spsr_el1", out(reg) spsr);
        core::arch::asm!("mrs {0}, CurrentEL", out(reg) current_el);
    }
    let ec = (esr >> 26) & 0x3f;
    let iss = esr & 0x1ffffff;
    uart::write_str("exception vector: 0x");
    uart::write_u64_hex(vector);
    uart::write_str("\n");
    uart::write_str("current_el: 0x");
    uart::write_u64_hex(current_el);
    uart::write_str(" ec: 0x");
    uart::write_u64_hex(ec);
    uart::write_str(" iss: 0x");
    uart::write_u64_hex(iss);
    uart::write_str("\n");
    uart::write_str("esr_el1: 0x");
    uart::write_u64_hex(esr);
    uart::write_str(" far_el1: 0x");
    uart::write_u64_hex(far);
    uart::write_str("\n");
    uart::write_str("elr_el1: 0x");
    uart::write_u64_hex(elr);
    uart::write_str(" spsr_el1: 0x");
    uart::write_u64_hex(spsr);
    uart::write_str("\n");
    loop {
        unsafe { core::arch::asm!("wfe"); }
    }
}

// Pull in the AArch64 entry point.
core::arch::global_asm!(include_str!("arch/aarch64/boot.S"));
