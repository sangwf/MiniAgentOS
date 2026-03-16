use core::arch::asm;

pub fn counter_freq_hz() -> u64 {
    let freq: u64;
    unsafe {
        asm!("mrs {0}, cntfrq_el0", out(reg) freq);
    }
    freq
}

pub fn counter_ticks() -> u64 {
    let ticks: u64;
    unsafe {
        asm!("mrs {0}, cntpct_el0", out(reg) ticks);
    }
    ticks
}

pub fn ticks_to_ms(ticks: u64, freq_hz: u64) -> u64 {
    if freq_hz == 0 {
        return 0;
    }
    ticks.saturating_mul(1_000) / freq_hz
}

pub fn delay_ms(ms: u64) {
    let freq = counter_freq_hz();
    let start = counter_ticks();
    let target = start.saturating_add(ms.saturating_mul(freq) / 1_000);
    while counter_ticks() < target {
        unsafe { asm!("nop"); }
    }
}
