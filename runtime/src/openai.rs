include!(concat!(env!("OUT_DIR"), "/openai_secrets.rs"));

static mut OPENAI_API_KEY: [u8; 256] = [0u8; 256];
static mut OPENAI_API_KEY_LEN: usize = 0;

const OPENAI_RESPONSES_URL: &[u8] = b"https://api.openai.com/v1/responses";
const OPENAI_RESPONSES_HOST: &[u8] = b"api.openai.com";
const OPENAI_MODEL: &[u8] = b"gpt-5-mini";

pub fn responses_url() -> &'static [u8] {
    OPENAI_RESPONSES_URL
}

pub fn responses_host() -> &'static [u8] {
    OPENAI_RESPONSES_HOST
}

pub fn model_name() -> &'static [u8] {
    OPENAI_MODEL
}

fn ascii_lower(b: u8) -> u8 {
    if b >= b'A' && b <= b'Z' {
        b + 32
    } else {
        b
    }
}

fn ascii_eq_ignore_case(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut i = 0usize;
    while i < left.len() {
        if ascii_lower(left[i]) != ascii_lower(right[i]) {
            return false;
        }
        i += 1;
    }
    true
}

pub fn is_responses_target(domain: &[u8], path: &[u8], https: bool, port: u16) -> bool {
    https
        && port == 443
        && ascii_eq_ignore_case(domain, OPENAI_RESPONSES_HOST)
        && path == b"/v1/responses"
}

pub fn api_key_ready() -> bool {
    unsafe { OPENAI_API_KEY_LEN != 0 }
}

pub fn init_embedded_api_key() {
    if !OPENAI_EMBEDDED_API_KEY_READY || api_key_ready() {
        return;
    }
    let _ = set_api_key(OPENAI_EMBEDDED_API_KEY);
}

pub fn clear_api_key() {
    unsafe {
        let mut i = 0usize;
        while i < OPENAI_API_KEY_LEN && i < OPENAI_API_KEY.len() {
            OPENAI_API_KEY[i] = 0;
            i += 1;
        }
        OPENAI_API_KEY_LEN = 0;
    }
}

pub fn set_api_key(key: &[u8]) -> bool {
    if key.is_empty() || key.len() > unsafe { OPENAI_API_KEY.len() } {
        return false;
    }
    unsafe {
        let mut i = 0usize;
        while i < key.len() {
            let b = key[i];
            if b == b'\r' || b == b'\n' {
                return false;
            }
            OPENAI_API_KEY[i] = b;
            i += 1;
        }
        while i < OPENAI_API_KEY_LEN && i < OPENAI_API_KEY.len() {
            OPENAI_API_KEY[i] = 0;
            i += 1;
        }
        OPENAI_API_KEY_LEN = key.len();
    }
    true
}

pub fn build_bearer_header(out: &mut [u8]) -> usize {
    if !api_key_ready() {
        return 0;
    }
    let prefix = b"Authorization: Bearer ";
    let suffix = b"\r\n";
    let key_len = unsafe { OPENAI_API_KEY_LEN };
    let needed = prefix.len() + key_len + suffix.len();
    if needed > out.len() {
        return 0;
    }
    let mut idx = 0usize;
    let mut i = 0usize;
    while i < prefix.len() {
        out[idx] = prefix[i];
        idx += 1;
        i += 1;
    }
    i = 0;
    while i < key_len {
        out[idx] = unsafe { OPENAI_API_KEY[i] };
        idx += 1;
        i += 1;
    }
    i = 0;
    while i < suffix.len() {
        out[idx] = suffix[i];
        idx += 1;
        i += 1;
    }
    idx
}
