use core::cmp::min;
use core::ffi::c_void;

use crate::net;
use crate::timer;

const MBEDTLS_ERR_SSL_WANT_READ: i32 = -0x6900;
const MBEDTLS_ERR_SSL_WANT_WRITE: i32 = -0x6880;
const MBEDTLS_ERR_SSL_PEER_CLOSE_NOTIFY: i32 = -0x7880;
const TLS_TX_CHUNK_LEN: usize = 1400;

static mut TLS_INIT: bool = false;
static mut TLS_HOST_BUF: [u8; 128] = [0u8; 128];
static mut TLS_RX_BUF: [u8; 65536] = [0u8; 65536];
static mut TLS_RX_LEN: usize = 0;
static mut TLS_RX_OFF: usize = 0;
const TLS_PENDING_SLOTS: usize = 4;
const TLS_PENDING_MAX: usize = 2048;
static mut TLS_PENDING_BUF: [[u8; TLS_PENDING_MAX]; TLS_PENDING_SLOTS] = [[0u8; TLS_PENDING_MAX]; TLS_PENDING_SLOTS];
static mut TLS_PENDING_LEN: [usize; TLS_PENDING_SLOTS] = [0usize; TLS_PENDING_SLOTS];
static mut TLS_PENDING_SEQ: [u32; TLS_PENDING_SLOTS] = [0u32; TLS_PENDING_SLOTS];
static mut TLS_PENDING_VALID: [bool; TLS_PENDING_SLOTS] = [false; TLS_PENDING_SLOTS];

#[repr(C)]
struct TlsBio {
    nb: usize,
    mac: [u8; 6],
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    gw_mac: [u8; 6],
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
}

static mut TLS_BIO: TlsBio = TlsBio {
    nb: 0,
    mac: [0u8; 6],
    src_ip: [0u8; 4],
    dst_ip: [0u8; 4],
    gw_mac: [0u8; 6],
    src_port: 0,
    dst_port: 0,
    seq: 0,
    ack: 0,
};

extern "C" {
    fn minios_tls_init() -> i32;
    fn minios_tls_reset() -> i32;
    fn minios_tls_set_bio(
        ctx: *mut c_void,
        f_send: Option<extern "C" fn(*mut c_void, *const u8, usize) -> i32>,
        f_recv: Option<extern "C" fn(*mut c_void, *mut u8, usize) -> i32>,
    );
    fn minios_tls_set_hostname(host: *const u8) -> i32;
    fn minios_tls_last_x509_err() -> i32;
    fn minios_tls_last_curve() -> i32;
    fn minios_tls_last_skx_err() -> i32;
    fn minios_tls_last_skx_ret() -> i32;
    fn minios_tls_diag_clear();
    fn minios_tls_handshake() -> i32;
    fn minios_tls_write(buf: *const u8, len: usize) -> i32;
    fn minios_tls_read(buf: *mut u8, len: usize) -> i32;
}

#[no_mangle]
pub extern "C" fn minios_entropy_fill(out: *mut u8, len: usize) -> i32 {
    if out.is_null() {
        return -1;
    }
    let mut state = unsafe { timer::counter_ticks() } ^ 0x9e37_79b9_7f4a_7c15;
    let mut i = 0usize;
    while i < len {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        unsafe { *out.add(i) = (state >> 32) as u8; }
        i += 1;
    }
    0
}

extern "C" fn tls_send(ctx: *mut c_void, buf: *const u8, len: usize) -> i32 {
    if ctx.is_null() || buf.is_null() {
        return -1;
    }
    let bio = unsafe { &mut *(ctx as *mut TlsBio) };
    let payload = unsafe { core::slice::from_raw_parts(buf, len) };
    let safe_chunk = core::cmp::min(TLS_TX_CHUNK_LEN, net::max_tcp_payload_len());
    if safe_chunk == 0 {
        return -1;
    }
    let send_len = core::cmp::min(payload.len(), safe_chunk);
    let before = net::tx_used_idx();
    net::send_tcp(
        bio.nb,
        bio.mac,
        bio.src_ip,
        bio.src_port,
        bio.gw_mac,
        bio.dst_ip,
        bio.dst_port,
        bio.seq,
        bio.ack,
        0x18,
        &payload[..send_len],
    );
    let mut spins = 0u32;
    while net::tx_used_idx() == before {
        spins = spins.wrapping_add(1);
        if spins > 200_000 {
            return MBEDTLS_ERR_SSL_WANT_WRITE;
        }
    }
    bio.seq = bio.seq.wrapping_add(send_len as u32);
    send_len as i32
}

extern "C" fn tls_recv(ctx: *mut c_void, buf: *mut u8, len: usize) -> i32 {
    if ctx.is_null() || buf.is_null() {
        return -1;
    }
    let available = unsafe { TLS_RX_LEN.saturating_sub(TLS_RX_OFF) };
    if available == 0 {
        return MBEDTLS_ERR_SSL_WANT_READ;
    }
    let n = min(available, len);
    unsafe {
        let src = TLS_RX_BUF.as_ptr().add(TLS_RX_OFF);
        let dst = buf;
        core::ptr::copy_nonoverlapping(src, dst, n);
        TLS_RX_OFF += n;
        if TLS_RX_OFF >= TLS_RX_LEN {
            TLS_RX_OFF = 0;
            TLS_RX_LEN = 0;
        }
    }
    n as i32
}

pub fn init_once() -> bool {
    unsafe {
        if TLS_INIT {
            return true;
        }
        crate::mem::mbedtls_heap_reset();
        let ret = minios_tls_init();
        TLS_INIT = ret == 0;
        TLS_INIT
    }
}

pub fn configure(
    nb: usize,
    mac: [u8; 6],
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    gw_mac: [u8; 6],
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    domain: &[u8],
) -> bool {
    if !init_once() {
        return false;
    }
    unsafe {
        let _ = minios_tls_reset();
        minios_tls_diag_clear();
        TLS_BIO = TlsBio {
            nb,
            mac,
            src_ip,
            dst_ip,
            gw_mac,
            src_port,
            dst_port,
            seq,
            ack,
        };
        TLS_RX_LEN = 0;
        TLS_RX_OFF = 0;
        let mut p = 0usize;
        while p < TLS_PENDING_SLOTS {
            TLS_PENDING_LEN[p] = 0;
            TLS_PENDING_SEQ[p] = 0;
            TLS_PENDING_VALID[p] = false;
            p += 1;
        }
        let mut i = 0usize;
        while i < TLS_HOST_BUF.len() && i < domain.len() {
            TLS_HOST_BUF[i] = domain[i];
            i += 1;
        }
        if i < TLS_HOST_BUF.len() {
            TLS_HOST_BUF[i] = 0;
        } else {
            TLS_HOST_BUF[TLS_HOST_BUF.len() - 1] = 0;
        }
        minios_tls_set_bio(&mut TLS_BIO as *mut TlsBio as *mut c_void, Some(tls_send), Some(tls_recv));
        minios_tls_set_hostname(TLS_HOST_BUF.as_ptr());
    }
    true
}

pub fn hard_reset() {
    unsafe {
        if TLS_INIT {
            let _ = minios_tls_reset();
            minios_tls_diag_clear();
        }
        TLS_RX_LEN = 0;
        TLS_RX_OFF = 0;
        let mut p = 0usize;
        while p < TLS_PENDING_SLOTS {
            TLS_PENDING_LEN[p] = 0;
            TLS_PENDING_SEQ[p] = 0;
            TLS_PENDING_VALID[p] = false;
            p += 1;
        }
    }
}

pub fn push_rx_payload(addr: usize, len: usize) {
    if len == 0 {
        return;
    }
    unsafe {
        if TLS_RX_OFF >= TLS_RX_LEN {
            TLS_RX_LEN = 0;
            TLS_RX_OFF = 0;
        }
        if TLS_RX_LEN + len > TLS_RX_BUF.len() && TLS_RX_OFF > 0 {
            let remaining = TLS_RX_LEN.saturating_sub(TLS_RX_OFF);
            if remaining > 0 {
                core::ptr::copy(
                    TLS_RX_BUF.as_ptr().add(TLS_RX_OFF),
                    TLS_RX_BUF.as_mut_ptr(),
                    remaining,
                );
            }
            TLS_RX_LEN = remaining;
            TLS_RX_OFF = 0;
        }
        if TLS_RX_LEN + len > TLS_RX_BUF.len() {
            TLS_RX_LEN = 0;
            TLS_RX_OFF = 0;
            return;
        }
        let dst = TLS_RX_BUF.as_mut_ptr().add(TLS_RX_LEN);
        let mut i = 0usize;
        while i < len {
            *dst.add(i) = crate::mmio::read8(addr + i);
            i += 1;
        }
        TLS_RX_LEN += len;
    }
}

pub fn update_ack(ack: u32) {
    unsafe {
        TLS_BIO.ack = ack;
    }
}

pub fn expected_ack() -> u32 {
    unsafe { TLS_BIO.ack }
}

fn copy_from_rx(addr: usize, len: usize, dst: *mut u8) {
    let mut i = 0usize;
    while i < len {
        unsafe {
            *dst.add(i) = crate::mmio::read8(addr + i);
        }
        i += 1;
    }
}

pub fn push_rx_payload_seq(seq: u32, addr: usize, len: usize) -> bool {
    if len == 0 {
        return false;
    }
    let expected = expected_ack();
    unsafe {
        if seq == expected {
            push_rx_payload(addr, len);
            TLS_BIO.ack = expected.wrapping_add(len as u32);
            loop {
                let mut found = false;
                let mut slot = 0usize;
                while slot < TLS_PENDING_SLOTS {
                    if TLS_PENDING_VALID[slot] && TLS_PENDING_SEQ[slot] == TLS_BIO.ack {
                        let pending_len = TLS_PENDING_LEN[slot];
                        if pending_len > 0 {
                            let base = TLS_PENDING_BUF[slot].as_ptr() as usize;
                            push_rx_payload(base, pending_len);
                            TLS_BIO.ack = TLS_BIO.ack.wrapping_add(pending_len as u32);
                        }
                        TLS_PENDING_VALID[slot] = false;
                        TLS_PENDING_LEN[slot] = 0;
                        found = true;
                        break;
                    }
                    slot += 1;
                }
                if !found {
                    break;
                }
            }
            return true;
        }
        if seq.wrapping_sub(expected) < 0x8000_0000 {
            if len <= TLS_PENDING_MAX {
                let mut slot = 0usize;
                while slot < TLS_PENDING_SLOTS {
                    if TLS_PENDING_VALID[slot] && TLS_PENDING_SEQ[slot] == seq {
                        return false;
                    }
                    slot += 1;
                }
                slot = 0;
                while slot < TLS_PENDING_SLOTS {
                    if !TLS_PENDING_VALID[slot] {
                        copy_from_rx(addr, len, TLS_PENDING_BUF[slot].as_mut_ptr());
                        TLS_PENDING_LEN[slot] = len;
                        TLS_PENDING_SEQ[slot] = seq;
                        TLS_PENDING_VALID[slot] = true;
                        break;
                    }
                    slot += 1;
                }
            }
            return false;
        }
    }
    false
}

pub fn send_ack() {
    unsafe {
        net::send_tcp(
            TLS_BIO.nb,
            TLS_BIO.mac,
            TLS_BIO.src_ip,
            TLS_BIO.src_port,
            TLS_BIO.gw_mac,
            TLS_BIO.dst_ip,
            TLS_BIO.dst_port,
            TLS_BIO.seq,
            TLS_BIO.ack,
            0x10,
            &[],
        );
    }
}

pub fn handshake_step() -> i32 {
    unsafe { minios_tls_handshake() }
}

pub fn write_step(buf: &[u8]) -> i32 {
    if buf.is_empty() {
        return 0;
    }
    unsafe { minios_tls_write(buf.as_ptr(), buf.len()) }
}

pub fn read_step(buf: &mut [u8]) -> i32 {
    if buf.is_empty() {
        return 0;
    }
    unsafe { minios_tls_read(buf.as_mut_ptr(), buf.len()) }
}

pub fn want_retry(code: i32) -> bool {
    code == MBEDTLS_ERR_SSL_WANT_READ || code == MBEDTLS_ERR_SSL_WANT_WRITE
}

pub fn is_peer_close(code: i32) -> bool {
    code == MBEDTLS_ERR_SSL_PEER_CLOSE_NOTIFY
}

pub fn debug_diag() -> (i32, i32) {
    unsafe { (minios_tls_last_x509_err(), minios_tls_last_curve()) }
}

pub fn debug_skx_err() -> i32 {
    unsafe { minios_tls_last_skx_err() }
}

pub fn debug_skx_ret() -> i32 {
    unsafe { minios_tls_last_skx_ret() }
}
