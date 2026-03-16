use crate::mmio;

const UART0_BASE: usize = 0x0900_0000;
const UART0_DR: usize = UART0_BASE + 0x00;
const UART0_FR: usize = UART0_BASE + 0x18;
const UART0_IBRD: usize = UART0_BASE + 0x24;
const UART0_FBRD: usize = UART0_BASE + 0x28;
const UART0_LCRH: usize = UART0_BASE + 0x2c;
const UART0_CR: usize = UART0_BASE + 0x30;
const UART0_ICR: usize = UART0_BASE + 0x44;

const UART0_FR_TXFF: u32 = 1 << 5;
const UART0_FR_RXFE: u32 = 1 << 4;
static mut UART_SILENT: bool = false;

pub fn init() {
    unsafe {
        // Disable UART.
        mmio::write32(UART0_CR, 0);
        // Clear interrupts.
        mmio::write32(UART0_ICR, 0x7ff);
        // Baud rate: 24_000_000 / (16 * 115200) = 13.0208
        mmio::write32(UART0_IBRD, 13);
        mmio::write32(UART0_FBRD, 2);
        // 8N1, FIFO enabled.
        mmio::write32(UART0_LCRH, (3 << 5) | (1 << 4));
        // Enable UART, TXE, RXE.
        mmio::write32(UART0_CR, (1 << 0) | (1 << 8) | (1 << 9));
    }
}

pub fn write_byte(b: u8) {
    unsafe {
        if UART_SILENT {
            return;
        }
    }
    unsafe {
        while (mmio::read32(UART0_FR) & UART0_FR_TXFF) != 0 {}
        mmio::write32(UART0_DR, b as u32);
    }
}

pub fn set_silent(silent: bool) {
    unsafe {
        UART_SILENT = silent;
    }
}

pub fn write_str(s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            write_byte(b'\r');
        }
        write_byte(b);
    }
}

pub fn write_bytes(bytes: &[u8]) {
    for &b in bytes {
        write_byte(b);
    }
}

pub fn write_u64_dec(mut n: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    if n == 0 {
        write_byte(b'0');
        return;
    }
    while n > 0 {
        let digit = (n % 10) as u8;
        i -= 1;
        buf[i] = b'0' + digit;
        n /= 10;
    }
    for &b in &buf[i..] {
        write_byte(b);
    }
}

pub fn write_u64_hex(n: u64) {
    let mut buf = [0u8; 16];
    for i in 0..16 {
        let shift = (15 - i) * 4;
        let digit = ((n >> shift) & 0xF) as u8;
        buf[i] = match digit {
            0..=9 => b'0' + digit,
            _ => b'a' + (digit - 10),
        };
    }
    for &b in &buf {
        write_byte(b);
    }
}

pub fn read_byte() -> Option<u8> {
    unsafe {
        if (mmio::read32(UART0_FR) & UART0_FR_RXFE) != 0 {
            return None;
        }
        Some(mmio::read32(UART0_DR) as u8)
    }
}

#[no_mangle]
pub extern "C" fn minios_uart_write(buf: *const u8, len: usize) {
    if buf.is_null() || len == 0 {
        return;
    }
    let mut i = 0usize;
    while i < len {
        unsafe {
            write_byte(*buf.add(i));
        }
        i += 1;
    }
}
