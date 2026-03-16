use core::arch::asm;

// NOTE: For bare-metal DMA/MMIO memory, avoid `core::ptr::read_volatile`.
// Rust UB precondition checks can panic/abort in-kernel. Use these inline-asm
// helpers for all device registers, virtio rings, and RX/TX buffers.

#[inline(always)]
pub unsafe fn read32(addr: usize) -> u32 {
    let val: u32;
    asm!(
        "ldr {val:w}, [{addr}]",
        addr = in(reg) addr,
        val = out(reg) val,
        options(nostack, preserves_flags)
    );
    val
}

#[inline(always)]
pub unsafe fn read8(addr: usize) -> u8 {
    let val: u8;
    asm!(
        "ldrb {val:w}, [{addr}]",
        addr = in(reg) addr,
        val = out(reg) val,
        options(nostack, preserves_flags)
    );
    val
}

#[inline(always)]
pub unsafe fn read16(addr: usize) -> u16 {
    let val: u16;
    asm!(
        "ldrh {val:w}, [{addr}]",
        addr = in(reg) addr,
        val = out(reg) val,
        options(nostack, preserves_flags)
    );
    val
}

#[inline(always)]
pub unsafe fn read64(addr: usize) -> u64 {
    let val: u64;
    asm!(
        "ldr {val}, [{addr}]",
        addr = in(reg) addr,
        val = out(reg) val,
        options(nostack, preserves_flags)
    );
    val
}

#[inline(always)]
pub unsafe fn write32(addr: usize, val: u32) {
    asm!(
        "str {val:w}, [{addr}]",
        addr = in(reg) addr,
        val = in(reg) val,
        options(nostack, preserves_flags)
    );
}

#[inline(always)]
pub fn barrier() {
    unsafe { asm!("dsb sy", "dmb sy", options(nostack, preserves_flags)) }
}

#[inline(always)]
pub unsafe fn store64(addr: *mut u64, val: u64) {
    asm!(
        "str {val}, [{addr}]",
        addr = in(reg) addr,
        val = in(reg) val,
        options(nostack, preserves_flags)
    );
}

#[inline(always)]
pub unsafe fn store32(addr: *mut u32, val: u32) {
    asm!(
        "str {val:w}, [{addr}]",
        addr = in(reg) addr,
        val = in(reg) val,
        options(nostack, preserves_flags)
    );
}

#[inline(always)]
pub unsafe fn store16(addr: *mut u16, val: u16) {
    asm!(
        "strh {val:w}, [{addr}]",
        addr = in(reg) addr,
        val = in(reg) val,
        options(nostack, preserves_flags)
    );
}

#[inline(always)]
pub unsafe fn store8(addr: *mut u8, val: u8) {
    asm!(
        "strb {val:w}, [{addr}]",
        addr = in(reg) addr,
        val = in(reg) val,
        options(nostack, preserves_flags)
    );
}
