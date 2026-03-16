use crate::timer;

include!(concat!(env!("OUT_DIR"), "/x_secrets.rs"));

extern "C" {
    fn minios_hmac_sha1(
        key: *const u8,
        key_len: usize,
        msg: *const u8,
        msg_len: usize,
        out20: *mut u8,
    ) -> i32;
    fn minios_base64_encode(
        src: *const u8,
        slen: usize,
        dst: *mut u8,
        dlen: usize,
        out_len: *mut usize,
    ) -> i32;
}

pub fn secrets_ready() -> bool {
    X_SECRETS_READY
}

pub fn bearer_token_ready() -> bool {
    X_BEARER_TOKEN_READY
}

pub fn build_bearer_header(out: &mut [u8]) -> usize {
    if !X_BEARER_TOKEN_READY {
        return 0;
    }
    let prefix = b"Authorization: Bearer ";
    let suffix = b"\r\n";
    let needed = prefix.len() + X_BEARER_TOKEN.len() + suffix.len();
    if needed > out.len() {
        return 0;
    }
    let mut idx = 0usize;
    idx = append_bytes(out, idx, prefix);
    idx = append_bytes(out, idx, X_BEARER_TOKEN);
    idx = append_bytes(out, idx, suffix);
    idx
}

pub fn now_timestamp(epoch_sec: u64, epoch_ticks: u64) -> u64 {
    let ticks = timer::counter_ticks();
    let freq = timer::counter_freq_hz();
    let delta = if ticks >= epoch_ticks {
        ticks - epoch_ticks
    } else {
        0
    };
    if epoch_sec == 0 || freq == 0 {
        timer::ticks_to_ms(ticks, timer::counter_freq_hz()) / 1000
    } else {
        epoch_sec + (delta / freq)
    }
}

pub fn nonce(counter: &mut u64) -> u64 {
    let ticks = timer::counter_ticks();
    *counter = counter.wrapping_add(1);
    ticks ^ (*counter << 1) ^ 0x9e37_79b9_7f4a_7c15
}

pub fn build_oauth_header(
    method: &[u8],
    base_url: &[u8],
    timestamp: u64,
    nonce: u64,
    out: &mut [u8],
) -> usize {
    if !X_SECRETS_READY {
        return 0;
    }
    let mut ts_buf = [0u8; 32];
    let ts_len = u64_to_dec(timestamp, &mut ts_buf);
    let mut nonce_buf = [0u8; 32];
    let nonce_len = u64_to_dec(nonce, &mut nonce_buf);

    let mut param_buf = [0u8; 512];
    let mut p = 0usize;
    p = append_param(&mut param_buf, p, b"oauth_consumer_key", X_API_KEY);
    p = append_param(&mut param_buf, p, b"oauth_nonce", &nonce_buf[..nonce_len]);
    p = append_param(&mut param_buf, p, b"oauth_signature_method", b"HMAC-SHA1");
    p = append_param(&mut param_buf, p, b"oauth_timestamp", &ts_buf[..ts_len]);
    p = append_param(&mut param_buf, p, b"oauth_token", X_ACCESS_TOKEN);
    p = append_param(&mut param_buf, p, b"oauth_version", b"1.0");
    let param_str = &param_buf[..p];

    let mut base_buf = [0u8; 1024];
    let mut b = 0usize;
    b = append_bytes(&mut base_buf, b, method);
    b = append_byte(&mut base_buf, b, b'&');
    let mut enc = [0u8; 512];
    let enc_len = percent_encode(base_url, &mut enc);
    b = append_bytes(&mut base_buf, b, &enc[..enc_len]);
    b = append_byte(&mut base_buf, b, b'&');
    let enc_param_len = percent_encode(param_str, &mut enc);
    b = append_bytes(&mut base_buf, b, &enc[..enc_param_len]);
    let base_str = &base_buf[..b];

    let mut key_buf = [0u8; 256];
    let mut k = 0usize;
    let enc_key_len = percent_encode(X_API_SECRET, &mut enc);
    k = append_bytes(&mut key_buf, k, &enc[..enc_key_len]);
    k = append_byte(&mut key_buf, k, b'&');
    let enc_tok_len = percent_encode(X_ACCESS_SECRET, &mut enc);
    k = append_bytes(&mut key_buf, k, &enc[..enc_tok_len]);
    let key_str = &key_buf[..k];

    let mut sig = [0u8; 20];
    let hret = unsafe {
        minios_hmac_sha1(
            key_str.as_ptr(),
            key_str.len(),
            base_str.as_ptr(),
            base_str.len(),
            sig.as_mut_ptr(),
        )
    };
    if hret != 0 {
        return 0;
    }

    let mut sig_b64 = [0u8; 64];
    let mut sig_b64_len = 0usize;
    let bret = unsafe {
        minios_base64_encode(
            sig.as_ptr(),
            sig.len(),
            sig_b64.as_mut_ptr(),
            sig_b64.len(),
            &mut sig_b64_len as *mut usize,
        )
    };
    if bret != 0 {
        return 0;
    }

    let mut i = 0usize;
    i = append_bytes(out, i, b"Authorization: OAuth ");
    i = append_kv(out, i, b"oauth_consumer_key", X_API_KEY, true);
    i = append_kv(out, i, b"oauth_nonce", &nonce_buf[..nonce_len], true);
    i = append_kv(out, i, b"oauth_signature", &sig_b64[..sig_b64_len], true);
    i = append_kv(out, i, b"oauth_signature_method", b"HMAC-SHA1", true);
    i = append_kv(out, i, b"oauth_timestamp", &ts_buf[..ts_len], true);
    i = append_kv(out, i, b"oauth_token", X_ACCESS_TOKEN, true);
    i = append_kv(out, i, b"oauth_version", b"1.0", false);
    if i + 2 <= out.len() {
        out[i] = b'\r';
        out[i + 1] = b'\n';
        i += 2;
    }
    i
}

fn append_param(out: &mut [u8], mut idx: usize, key: &[u8], val: &[u8]) -> usize {
    if idx > 0 {
        idx = append_byte(out, idx, b'&');
    }
    idx = append_bytes(out, idx, key);
    idx = append_byte(out, idx, b'=');
    let mut tmp = [0u8; 256];
    let n = percent_encode(val, &mut tmp);
    idx = append_bytes(out, idx, &tmp[..n]);
    idx
}

fn append_kv(out: &mut [u8], mut idx: usize, key: &[u8], val: &[u8], comma: bool) -> usize {
    idx = append_bytes(out, idx, key);
    idx = append_bytes(out, idx, b"=\"");
    let mut tmp = [0u8; 256];
    let n = percent_encode(val, &mut tmp);
    idx = append_bytes(out, idx, &tmp[..n]);
    idx = append_byte(out, idx, b'"');
    if comma {
        idx = append_bytes(out, idx, b", ");
    }
    idx
}

fn percent_encode(input: &[u8], out: &mut [u8]) -> usize {
    let mut i = 0usize;
    let mut j = 0usize;
    while i < input.len() && j < out.len() {
        let b = input[i];
        if is_unreserved(b) {
            out[j] = b;
            j += 1;
        } else {
            if j + 3 > out.len() {
                break;
            }
            out[j] = b'%';
            out[j + 1] = hex_upper(b >> 4);
            out[j + 2] = hex_upper(b & 0x0f);
            j += 3;
        }
        i += 1;
    }
    j
}

fn is_unreserved(b: u8) -> bool {
    (b'A'..=b'Z').contains(&b)
        || (b'a'..=b'z').contains(&b)
        || (b'0'..=b'9').contains(&b)
        || b == b'-'
        || b == b'.'
        || b == b'_'
        || b == b'~'
}

fn hex_upper(b: u8) -> u8 {
    match b {
        0..=9 => b'0' + b,
        _ => b'A' + (b - 10),
    }
}

fn u64_to_dec(mut v: u64, out: &mut [u8]) -> usize {
    let mut tmp = [0u8; 20];
    let mut n = 0usize;
    if v == 0 {
        if !out.is_empty() {
            out[0] = b'0';
            return 1;
        }
        return 0;
    }
    while v > 0 && n < tmp.len() {
        tmp[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    let mut i = 0usize;
    while i < n && i < out.len() {
        out[i] = tmp[n - 1 - i];
        i += 1;
    }
    i
}

fn append_bytes(out: &mut [u8], mut idx: usize, src: &[u8]) -> usize {
    let mut i = 0usize;
    while i < src.len() && idx < out.len() {
        out[idx] = src[i];
        idx += 1;
        i += 1;
    }
    idx
}

fn append_byte(out: &mut [u8], idx: usize, b: u8) -> usize {
    if idx < out.len() {
        out[idx] = b;
        return idx + 1;
    }
    idx
}
