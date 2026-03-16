#[no_mangle]
pub unsafe extern "C" fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0usize;
    while i < n {
        *dst.add(i) = *src.add(i);
        i += 1;
    }
    dst
}

#[no_mangle]
pub unsafe extern "C" fn memmove(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if (dst as usize) <= (src as usize) {
        memcpy(dst, src, n)
    } else {
        let mut i = n;
        while i > 0 {
            i -= 1;
            *dst.add(i) = *src.add(i);
        }
        dst
    }
}

#[no_mangle]
pub unsafe extern "C" fn memset(dst: *mut u8, c: i32, n: usize) -> *mut u8 {
    let mut i = 0usize;
    let v = c as u8;
    while i < n {
        *dst.add(i) = v;
        i += 1;
    }
    dst
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    let mut i = 0usize;
    while i < n {
        let va = *a.add(i);
        let vb = *b.add(i);
        if va != vb {
            return (va as i32) - (vb as i32);
        }
        i += 1;
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const u8) -> usize {
    if s.is_null() {
        return 0;
    }
    let mut n = 0usize;
    while *s.add(n) != 0 {
        n += 1;
    }
    n
}

#[no_mangle]
pub unsafe extern "C" fn strcmp(a: *const u8, b: *const u8) -> i32 {
    if a.is_null() || b.is_null() {
        return if a.is_null() && b.is_null() { 0 } else if a.is_null() { -1 } else { 1 };
    }
    let mut i = 0usize;
    loop {
        let va = *a.add(i);
        let vb = *b.add(i);
        if va != vb {
            return (va as i32) - (vb as i32);
        }
        if va == 0 {
            return 0;
        }
        i += 1;
    }
}

#[no_mangle]
pub unsafe extern "C" fn strchr(s: *const u8, c: i32) -> *mut u8 {
    if s.is_null() {
        return null_mut();
    }
    let needle = c as u8;
    let mut i = 0usize;
    loop {
        let ch = *s.add(i);
        if ch == needle {
            return s.add(i) as *mut u8;
        }
        if ch == 0 {
            return null_mut();
        }
        i += 1;
    }
}

const MBEDTLS_HEAP_SIZE: usize = 16 * 1024 * 1024;
#[repr(align(16))]
struct MbedtlsHeap([u8; MBEDTLS_HEAP_SIZE]);
static mut MBEDTLS_HEAP: MbedtlsHeap = MbedtlsHeap([0u8; MBEDTLS_HEAP_SIZE]);
static mut MBEDTLS_HEAP_OFF: usize = 0;

pub fn mbedtls_heap_reset() {
    unsafe {
        MBEDTLS_HEAP_OFF = 0;
    }
}

#[no_mangle]
pub unsafe extern "C" fn calloc(nmemb: usize, size: usize) -> *mut u8 {
    let total = match nmemb.checked_mul(size) {
        Some(v) => v,
        None => return null_mut(),
    };
    if total == 0 {
        return null_mut();
    }
    let align = 16usize;
    let base = MBEDTLS_HEAP.0.as_ptr() as usize;
    let mut off = MBEDTLS_HEAP_OFF;
    if off == 0 {
        off = base;
    }
    let aligned = (off + align - 1) & !(align - 1);
    let end = match aligned.checked_add(total) {
        Some(v) => v,
        None => return null_mut(),
    };
    if end > base + MBEDTLS_HEAP_SIZE {
        return null_mut();
    }
    MBEDTLS_HEAP_OFF = end;
    let ptr = aligned as *mut u8;
    let mut i = 0usize;
    while i < total {
        *ptr.add(i) = 0;
        i += 1;
    }
    ptr
}

#[no_mangle]
pub unsafe extern "C" fn free(_ptr: *mut u8) {}

#[no_mangle]
pub unsafe extern "C" fn time(out: *mut i64) -> i64 {
    if !out.is_null() {
        *out = 0;
    }
    0
}
use core::ptr::null_mut;
