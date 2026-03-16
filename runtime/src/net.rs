use crate::{mmio, timer, uart, virtio};

pub const NET_CONFIG_OFFSET: usize = 0x100;
pub const MAC_LEN: usize = 6;
pub const VIRTIO_NET_F_MAC: u64 = 1 << 5;

pub const VIRTQ_DESC_F_WRITE: u16 = 2;

const MAX_QSIZE: u16 = 256;
const LEGACY_QUEUE_ALIGN: usize = 4096;
const VIRTIO_NET_HDR_LEN: usize = 10;
const ETH_HDR_LEN: usize = 14;
const ARP_PKT_LEN: usize = 28;
const VIRTQ_HEAP_SIZE: usize = 128 * 1024;

static mut VIRTQ_HEAP: [u8; VIRTQ_HEAP_SIZE] = [0u8; VIRTQ_HEAP_SIZE];
static mut VIRTQ_HEAP_OFF: usize = 0;
const MMIO_GUEST_PAGE_SIZE: usize = 0x028;
const MMIO_QUEUE_SEL: usize = 0x030;
const MMIO_QUEUE_NUM_MAX: usize = 0x034;
const MMIO_QUEUE_NUM: usize = 0x038;
const MMIO_QUEUE_ALIGN: usize = 0x03c;
const MMIO_QUEUE_PFN: usize = 0x040;
const MMIO_QUEUE_READY_MODERN: usize = 0x044;
const MMIO_QUEUE_DESC_LOW: usize = 0x080;
const MMIO_QUEUE_DESC_HIGH: usize = 0x084;
const MMIO_QUEUE_AVAIL_LOW: usize = 0x090;
const MMIO_QUEUE_AVAIL_HIGH: usize = 0x094;
const MMIO_QUEUE_USED_LOW: usize = 0x0a0;
const MMIO_QUEUE_USED_HIGH: usize = 0x0a4;
const QUEUE_NOTIFY: usize = 0x050;
const MMIO_INTERRUPT_STATUS: usize = 0x060;
const MMIO_STATUS: usize = 0x070;
const MMIO_MAGIC_VALUE: usize = 0x000;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct VirtqDesc {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: u16,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct VirtqUsedElem {
    pub id: u32,
    pub len: u32,
}

const fn align_up(val: usize, align: usize) -> usize {
    (val + align - 1) & !(align - 1)
}

fn virtq_alloc(size: usize, align: usize) -> *mut u8 {
    unsafe {
        let base = core::ptr::addr_of!(VIRTQ_HEAP) as usize;
        let end = base + VIRTQ_HEAP_SIZE;
        let start = align_up(base + VIRTQ_HEAP_OFF, align);
        let next = match start.checked_add(size) {
            Some(v) => v,
            None => return core::ptr::null_mut(),
        };
        if next > end {
            return core::ptr::null_mut();
        }
        VIRTQ_HEAP_OFF = next - base;
        start as *mut u8
    }
}

pub struct Virtq {
    pub desc: *mut VirtqDesc,
    pub avail: *mut u8,
    pub used: *mut u8,
    pub qsize: u16,
    pub ring: *mut u8,
    pub ring_size: usize,
}

impl Virtq {
    pub const fn empty() -> Self {
        Self {
            desc: core::ptr::null_mut(),
            avail: core::ptr::null_mut(),
            used: core::ptr::null_mut(),
            qsize: 0,
            ring: core::ptr::null_mut(),
            ring_size: 0,
        }
    }
}

pub(crate) static mut RXQ: Virtq = Virtq::empty();
pub(crate) static mut TXQ: Virtq = Virtq::empty();

const RX_BUF_LEN: usize = 8192;
const RX_BUF_COUNT_MAX: usize = 8;
const TX_BUF_LEN: usize = 2048;
const TX_FRAME_OVERHEAD: usize = VIRTIO_NET_HDR_LEN + ETH_HDR_LEN + 20 + 20;
pub(crate) static mut TX_BUF: [u8; TX_BUF_LEN] = [0u8; TX_BUF_LEN];
static mut TX_LAST_SENT: u16 = 0;

static mut RX_BUF_PTR: *mut u8 = core::ptr::null_mut();
static mut RX_BUF_PTRS: [*mut u8; RX_BUF_COUNT_MAX] = [core::ptr::null_mut(); RX_BUF_COUNT_MAX];
static mut RX_BUF_COUNT: usize = 0;
static mut RX_CUR_ID: u16 = 0;
static mut RX_CUR_LEN: usize = 0;
static mut RX_CUR_VALID: bool = false;
static mut RX_NEEDS_REARM: bool = false;
static mut LAST_ARP_MAC: [u8; 6] = [0u8; 6];
static mut LAST_ARP_IP: [u8; 4] = [0u8; 4];
static mut LAST_ARP_VALID: bool = false;
const ARP_CACHE_SLOTS: usize = 8;
const ARP_CACHE_TTL_MS: u64 = 120_000;
static mut ARP_CACHE_MACS: [[u8; 6]; ARP_CACHE_SLOTS] = [[0u8; 6]; ARP_CACHE_SLOTS];
static mut ARP_CACHE_IPS: [[u8; 4]; ARP_CACHE_SLOTS] = [[0u8; 4]; ARP_CACHE_SLOTS];
static mut ARP_CACHE_EXPIRY_MS: [u64; ARP_CACHE_SLOTS] = [0u64; ARP_CACHE_SLOTS];
static mut ARP_CACHE_VALID: [bool; ARP_CACHE_SLOTS] = [false; ARP_CACHE_SLOTS];
static mut ARP_CACHE_NEXT: usize = 0;

fn arp_cache_now_ms() -> u64 {
    timer::ticks_to_ms(timer::counter_ticks(), timer::counter_freq_hz())
}

fn arp_cache_store(mac: [u8; 6], ip: [u8; 4]) {
    let now = arp_cache_now_ms();
    unsafe {
        let mut slot = 0usize;
        let mut target = None;
        while slot < ARP_CACHE_SLOTS {
            if ARP_CACHE_VALID[slot] && ARP_CACHE_IPS[slot] == ip {
                target = Some(slot);
                break;
            }
            if !ARP_CACHE_VALID[slot] && target.is_none() {
                target = Some(slot);
            }
            slot += 1;
        }
        let slot = target.unwrap_or_else(|| {
            let idx = ARP_CACHE_NEXT % ARP_CACHE_SLOTS;
            ARP_CACHE_NEXT = (ARP_CACHE_NEXT + 1) % ARP_CACHE_SLOTS;
            idx
        });
        ARP_CACHE_MACS[slot] = mac;
        ARP_CACHE_IPS[slot] = ip;
        ARP_CACHE_EXPIRY_MS[slot] = now + ARP_CACHE_TTL_MS;
        ARP_CACHE_VALID[slot] = true;
    }
}

fn current_rx_bounds(min_len: usize) -> Option<(usize, usize)> {
    unsafe {
        if !RX_CUR_VALID || RX_BUF_PTR.is_null() {
            return None;
        }
        let p = RX_BUF_PTR as usize;
        let heap_base = core::ptr::addr_of!(VIRTQ_HEAP) as usize;
        let heap_end = heap_base + VIRTQ_HEAP_SIZE;
        let p_end = match p.checked_add(RX_BUF_LEN) {
            Some(v) => v,
            None => {
                RX_CUR_VALID = false;
                RX_CUR_LEN = 0;
                RX_BUF_PTR = core::ptr::null_mut();
                return None;
            }
        };
        if p < heap_base || p_end > heap_end {
            RX_CUR_VALID = false;
            RX_CUR_LEN = 0;
            RX_BUF_PTR = core::ptr::null_mut();
            return None;
        }
        let mut found = false;
        let mut i = 0usize;
        while i < RX_BUF_COUNT {
            if RX_BUF_PTRS[i] as usize == p {
                found = true;
                break;
            }
            i += 1;
        }
        if !found {
            RX_CUR_VALID = false;
            RX_CUR_LEN = 0;
            RX_BUF_PTR = core::ptr::null_mut();
            return None;
        }
        let rx_len = if RX_CUR_LEN > RX_BUF_LEN {
            RX_BUF_LEN
        } else {
            RX_CUR_LEN
        };
        if rx_len < min_len {
            return None;
        }
        Some((p, rx_len))
    }
}

pub fn lookup_arp_peer(ip: [u8; 4]) -> Option<[u8; 6]> {
    let now = arp_cache_now_ms();
    unsafe {
        let mut slot = 0usize;
        while slot < ARP_CACHE_SLOTS {
            if ARP_CACHE_VALID[slot] {
                if ARP_CACHE_EXPIRY_MS[slot] <= now {
                    ARP_CACHE_VALID[slot] = false;
                } else if ARP_CACHE_IPS[slot] == ip {
                    return Some(ARP_CACHE_MACS[slot]);
                }
            }
            slot += 1;
        }
        None
    }
}

fn avail_flags_ptr(q: &Virtq) -> *mut u16 {
    q.avail as *mut u16
}

fn avail_idx_ptr(q: &Virtq) -> *mut u16 {
    unsafe { (q.avail as *mut u16).add(1) }
}

fn avail_ring_ptr(q: &Virtq) -> *mut u16 {
    unsafe { (q.avail as *mut u16).add(2) }
}

fn avail_used_event_ptr(q: &Virtq) -> *mut u16 {
    unsafe { avail_ring_ptr(q).add(q.qsize as usize) }
}

fn used_flags_ptr(q: &Virtq) -> *mut u16 {
    q.used as *mut u16
}

fn used_idx_ptr(q: &Virtq) -> *mut u16 {
    unsafe { (q.used as *mut u16).add(1) }
}

fn used_ring_ptr(q: &Virtq) -> *mut VirtqUsedElem {
    unsafe { (q.used as *mut u16).add(2) as *mut VirtqUsedElem }
}

fn used_avail_event_ptr(q: &Virtq) -> *mut u16 {
    unsafe { (used_ring_ptr(q) as *mut u8).add(core::mem::size_of::<VirtqUsedElem>() * q.qsize as usize) as *mut u16 }
}

fn alloc_virtq(qsize: u16, legacy: bool) -> Option<Virtq> {
    if qsize == 0 {
        return None;
    }
    let desc_size = core::mem::size_of::<VirtqDesc>() * qsize as usize;
    let avail_size = 4 + (qsize as usize * 2) + 2;
    let used_size = 4 + (core::mem::size_of::<VirtqUsedElem>() * qsize as usize) + 2;
    let used_align = if legacy { LEGACY_QUEUE_ALIGN } else { 4 };
    let used_off = align_up(desc_size + avail_size, used_align);
    let total = used_off + used_size;
    let ring_align = if legacy { LEGACY_QUEUE_ALIGN } else { 16 };
    uart::write_str("virtio-net alloc ring size: 0x");
    uart::write_u64_hex(total as u64);
    uart::write_str(" align: 0x");
    uart::write_u64_hex(ring_align as u64);
    uart::write_str("\n");
    let ring = virtq_alloc(total, ring_align);
    if ring.is_null() {
        uart::write_str("virtio-net alloc ring null\n");
        return None;
    }
    uart::write_str("virtio-net alloc ring addr: 0x");
    uart::write_u64_hex(ring as u64);
    uart::write_str("\n");
    let desc = ring as *mut VirtqDesc;
    let avail = unsafe { ring.add(desc_size) };
    let used = unsafe { ring.add(used_off) };
    let q = Virtq {
        desc,
        avail,
        used,
        qsize,
        ring,
        ring_size: total,
    };
    unsafe {
        mmio::store16(avail_flags_ptr(&q), 0);
        mmio::store16(avail_idx_ptr(&q), 0);
        mmio::store16(avail_used_event_ptr(&q), 0);
        mmio::store16(used_flags_ptr(&q), 0);
        mmio::store16(used_idx_ptr(&q), 0);
        mmio::store16(used_avail_event_ptr(&q), 0);
    }
    Some(q)
}

pub fn init_queues(mmio_base: usize, modern: bool) -> bool {
    dump_mmio_state(mmio_base, "virtio-net mmio pre-queue");
    if !modern {
        unsafe { mmio::write32(mmio_base + MMIO_GUEST_PAGE_SIZE, LEGACY_QUEUE_ALIGN as u32) };
    }
    let max0 = unsafe { mmio::read32(mmio_base + MMIO_QUEUE_NUM_MAX) } as u16;
    uart::write_str("virtio-net rx max: 0x");
    uart::write_u64_hex(max0 as u64);
    uart::write_str("\n");
    if max0 == 0 {
        return false;
    }
    let qsize = if max0 < MAX_QSIZE { max0 } else { MAX_QSIZE };

    let legacy = !modern;
    let rxq = match alloc_virtq(qsize, legacy) {
        Some(q) => q,
        None => {
            uart::write_str("virtio-net rx alloc fail\n");
            return false;
        }
    };
    let rx_count = if (qsize as usize) < RX_BUF_COUNT_MAX { qsize as usize } else { RX_BUF_COUNT_MAX };
    let mut actual_count = 0usize;
    let mut i = 0usize;
    while i < rx_count {
        let rx_buf = virtq_alloc(RX_BUF_LEN, 4096);
        if rx_buf.is_null() {
            break;
        }
        unsafe { RX_BUF_PTRS[i] = rx_buf };
        actual_count += 1;
        i += 1;
    }
    if actual_count == 0 {
        uart::write_str("virtio-net rx buf alloc fail\n");
        return false;
    }
    unsafe {
        RX_BUF_COUNT = actual_count;
        RX_BUF_PTR = RX_BUF_PTRS[0];
        RX_CUR_ID = 0;
        RX_CUR_VALID = true;
        RX_NEEDS_REARM = false;
    }
    uart::write_str("virtio-net rx alloc ok\n");
    // Queue 0: RX
    unsafe {
        RXQ = rxq;
        mmio::write32(mmio_base + MMIO_QUEUE_SEL, 0);
        dump_queue_sel(mmio_base, 0, "virtio-net rx queue sel");
        dump_queue_regs(mmio_base, 0, modern, "virtio-net rx queue pre-write");
        mmio::write32(mmio_base + MMIO_QUEUE_NUM, qsize as u32);
        dump_queue_num(mmio_base, 0, "virtio-net rx queue num");
        if !modern {
            mmio::write32(mmio_base + MMIO_QUEUE_ALIGN, LEGACY_QUEUE_ALIGN as u32);
        }
        dump_queue_regs(mmio_base, 0, modern, "virtio-net rx queue post-write");
        dump_mmio_state(mmio_base, "virtio-net mmio after rx queue writes");
        let ring = RXQ.ring as u64;
        if modern {
            let desc = RXQ.desc as u64;
            let avail = RXQ.avail as u64;
            let used = RXQ.used as u64;
            mmio::write32(mmio_base + MMIO_QUEUE_DESC_LOW, desc as u32);
            mmio::write32(mmio_base + MMIO_QUEUE_DESC_HIGH, (desc >> 32) as u32);
            mmio::write32(mmio_base + MMIO_QUEUE_AVAIL_LOW, avail as u32);
            mmio::write32(mmio_base + MMIO_QUEUE_AVAIL_HIGH, (avail >> 32) as u32);
            mmio::write32(mmio_base + MMIO_QUEUE_USED_LOW, used as u32);
            mmio::write32(mmio_base + MMIO_QUEUE_USED_HIGH, (used >> 32) as u32);
            mmio::write32(mmio_base + MMIO_QUEUE_READY_MODERN, 1);
            dump_queue_ready(mmio_base, 0, modern, "virtio-net rx queue ready");
            uart::write_str("virtio-net rx ring set (modern)\n");
            dump_queue_regs(mmio_base, 0, modern, "virtio-net rx queue after ready");
            dump_queue_addrs(mmio_base, 0, "virtio-net rx queue addrs");
        } else {
            mmio::write32(mmio_base + MMIO_QUEUE_PFN, (ring >> 12) as u32);
            uart::write_str("virtio-net rx ring set\n");
            uart::write_str("virtio-net rx pfn: 0x");
            uart::write_u64_hex(mmio::read32(mmio_base + MMIO_QUEUE_PFN) as u64);
            uart::write_str("\n");
            dump_queue_regs(mmio_base, 0, modern, "virtio-net rx queue after pfn");
        }
        uart::write_str("virtio-net rx desc set start\n");
        let mut j = 0usize;
        while j < actual_count {
            let d = unsafe { RXQ.desc.add(j) };
            let buf = unsafe { RX_BUF_PTRS[j] };
            mmio::store64(core::ptr::addr_of_mut!((*d).addr), buf as u64);
            mmio::store32(core::ptr::addr_of_mut!((*d).len), RX_BUF_LEN as u32);
            mmio::store16(core::ptr::addr_of_mut!((*d).flags), VIRTQ_DESC_F_WRITE);
            mmio::store16(core::ptr::addr_of_mut!((*d).next), 0);
            mmio::store16(avail_ring_ptr(&RXQ).add(j), j as u16);
            j += 1;
        }
        mmio::store16(avail_idx_ptr(&RXQ), actual_count as u16);
        uart::write_str("virtio-net rx desc set done\n");
        mmio::barrier();
        mmio::write32(mmio_base + QUEUE_NOTIFY, 0);
        uart::write_str("virtio-net rx notify\n");
    }

    let max1 = unsafe { mmio::read32(mmio_base + MMIO_QUEUE_NUM_MAX) as u16 };
    uart::write_str("virtio-net tx max: 0x");
    uart::write_u64_hex(max1 as u64);
    uart::write_str("\n");
    if max1 == 0 {
        return false;
    }
    let qsize1 = if max1 < MAX_QSIZE { max1 } else { MAX_QSIZE };
    let txq = match alloc_virtq(qsize1, legacy) {
        Some(q) => q,
        None => {
            uart::write_str("virtio-net tx alloc fail\n");
            return false;
        }
    };
    // Queue 1: TX
    unsafe {
        TXQ = txq;
        mmio::write32(mmio_base + MMIO_QUEUE_SEL, 1);
        dump_queue_sel(mmio_base, 1, "virtio-net tx queue sel");
        dump_queue_regs(mmio_base, 1, modern, "virtio-net tx queue pre-write");
        mmio::write32(mmio_base + MMIO_QUEUE_NUM, qsize1 as u32);
        dump_queue_num(mmio_base, 1, "virtio-net tx queue num");
        if !modern {
            mmio::write32(mmio_base + MMIO_QUEUE_ALIGN, LEGACY_QUEUE_ALIGN as u32);
        }
        dump_queue_regs(mmio_base, 1, modern, "virtio-net tx queue post-write");
        dump_mmio_state(mmio_base, "virtio-net mmio after tx queue writes");
        let ring = TXQ.ring as u64;
        if modern {
            let desc = TXQ.desc as u64;
            let avail = TXQ.avail as u64;
            let used = TXQ.used as u64;
            mmio::write32(mmio_base + MMIO_QUEUE_DESC_LOW, desc as u32);
            mmio::write32(mmio_base + MMIO_QUEUE_DESC_HIGH, (desc >> 32) as u32);
            mmio::write32(mmio_base + MMIO_QUEUE_AVAIL_LOW, avail as u32);
            mmio::write32(mmio_base + MMIO_QUEUE_AVAIL_HIGH, (avail >> 32) as u32);
            mmio::write32(mmio_base + MMIO_QUEUE_USED_LOW, used as u32);
            mmio::write32(mmio_base + MMIO_QUEUE_USED_HIGH, (used >> 32) as u32);
            mmio::write32(mmio_base + MMIO_QUEUE_READY_MODERN, 1);
            dump_queue_ready(mmio_base, 1, modern, "virtio-net tx queue ready");
            uart::write_str("virtio-net tx ring set (modern)\n");
            dump_queue_regs(mmio_base, 1, modern, "virtio-net tx queue after ready");
            dump_queue_addrs(mmio_base, 1, "virtio-net tx queue addrs");
        } else {
            mmio::write32(mmio_base + MMIO_QUEUE_PFN, (ring >> 12) as u32);
            uart::write_str("virtio-net tx ring set\n");
            uart::write_str("virtio-net tx pfn: 0x");
            uart::write_u64_hex(mmio::read32(mmio_base + MMIO_QUEUE_PFN) as u64);
            uart::write_str("\n");
            dump_queue_regs(mmio_base, 1, modern, "virtio-net tx queue after pfn");
        }
        TX_LAST_SENT = mmio::read16(used_idx_ptr(&TXQ) as usize);
    }
    true
}

fn tx_wait_ready() -> bool {
    let mut spins = 0u32;
    loop {
        let cur = tx_used_idx();
        unsafe {
            if cur == TX_LAST_SENT {
                return true;
            }
        }
        spins = spins.wrapping_add(1);
        if spins > 200_000 {
            return false;
        }
    }
}

pub fn send_dummy(mmio_base: usize) -> bool {
    if !tx_wait_ready() {
        return false;
    }
    let hdr_len = VIRTIO_NET_HDR_LEN;
    let frame_len = 60usize;
    let payload_len = hdr_len + frame_len;
    unsafe {
        uart::write_str("virtio-net tx buf fill\n");
        let p = core::ptr::addr_of_mut!(TX_BUF) as *mut u8;
        let mut i = 0usize;
        while i < payload_len {
            mmio::store8(p.add(i), 0u8);
            i += 1;
        }
        // Ethernet frame starts after virtio-net header.
        let frame = p.add(hdr_len);
        // dst mac: ff:ff:ff:ff:ff:ff (broadcast)
        for j in 0..6 {
            mmio::store8(frame.add(j), 0xff);
        }
        // src mac: 02:00:00:00:00:01
        mmio::store8(frame.add(6), 0x02);
        mmio::store8(frame.add(7), 0x00);
        mmio::store8(frame.add(8), 0x00);
        mmio::store8(frame.add(9), 0x00);
        mmio::store8(frame.add(10), 0x00);
        mmio::store8(frame.add(11), 0x01);
        // ethertype: ARP (0x0806)
        mmio::store8(frame.add(12), 0x08);
        mmio::store8(frame.add(13), 0x06);
        uart::write_str("virtio-net tx desc set\n");
        let d0 = TXQ.desc;
        mmio::store64(core::ptr::addr_of_mut!((*d0).addr), core::ptr::addr_of!(TX_BUF) as u64);
        mmio::store32(core::ptr::addr_of_mut!((*d0).len), payload_len as u32);
        mmio::store16(core::ptr::addr_of_mut!((*d0).flags), 0);
        mmio::store16(core::ptr::addr_of_mut!((*d0).next), 0);
        let cur = mmio::read16(avail_idx_ptr(&TXQ) as usize);
        let slot = (cur as usize) % TXQ.qsize as usize;
        mmio::store16(unsafe { avail_ring_ptr(&TXQ).add(slot) }, 0);
        mmio::store16(avail_idx_ptr(&TXQ), cur.wrapping_add(1));
        mmio::barrier();
        let before_used = mmio::read16(used_idx_ptr(&TXQ) as usize);
        uart::write_str("virtio-net tx used(before): 0x");
        uart::write_u64_hex(before_used as u64);
        uart::write_str("\n");
        uart::write_str("virtio-net tx notify\n");
        uart::write_str("virtio-net tx notify addr: 0x");
        uart::write_u64_hex((mmio_base + QUEUE_NOTIFY) as u64);
        uart::write_str("\n");
        mmio::write32(mmio_base + QUEUE_NOTIFY, 1);
        let isr = mmio::read32(mmio_base + MMIO_INTERRUPT_STATUS);
        uart::write_str("virtio-net isr: 0x");
        uart::write_u64_hex(isr as u64);
        uart::write_str("\n");
        let after_used = mmio::read16(used_idx_ptr(&TXQ) as usize);
        uart::write_str("virtio-net tx used(after): 0x");
        uart::write_u64_hex(after_used as u64);
        uart::write_str("\n");
        TX_LAST_SENT = cur.wrapping_add(1);
    }
    true
}

fn write_be16(buf: *mut u8, off: usize, val: u16) {
    unsafe {
        mmio::store8(buf.add(off), (val >> 8) as u8);
        mmio::store8(buf.add(off + 1), (val & 0xff) as u8);
    }
}

fn write_be32(buf: *mut u8, off: usize, val: u32) {
    unsafe {
        mmio::store8(buf.add(off), ((val >> 24) & 0xff) as u8);
        mmio::store8(buf.add(off + 1), ((val >> 16) & 0xff) as u8);
        mmio::store8(buf.add(off + 2), ((val >> 8) & 0xff) as u8);
        mmio::store8(buf.add(off + 3), (val & 0xff) as u8);
    }
}

fn checksum16_be(base: usize, len: usize) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0usize;
    while i + 1 < len {
        let hi = unsafe { mmio::read8(base + i) } as u32;
        let lo = unsafe { mmio::read8(base + i + 1) } as u32;
        sum = sum.wrapping_add((hi << 8) | lo);
        i += 2;
    }
    if i < len {
        let hi = unsafe { mmio::read8(base + i) } as u32;
        sum = sum.wrapping_add(hi << 8);
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

fn sum_be16_words(base: usize, len: usize) -> u32 {
    let mut sum: u32 = 0;
    let mut i = 0usize;
    while i + 1 < len {
        let hi = unsafe { mmio::read8(base + i) } as u32;
        let lo = unsafe { mmio::read8(base + i + 1) } as u32;
        sum = sum.wrapping_add((hi << 8) | lo);
        i += 2;
    }
    if i < len {
        let hi = unsafe { mmio::read8(base + i) } as u32;
        sum = sum.wrapping_add(hi << 8);
    }
    sum
}

fn checksum16_tcp(src_ip: [u8; 4], dst_ip: [u8; 4], tcp_base: usize, tcp_len: usize) -> u16 {
    let mut sum: u32 = 0;
    sum = sum.wrapping_add(((src_ip[0] as u32) << 8) | src_ip[1] as u32);
    sum = sum.wrapping_add(((src_ip[2] as u32) << 8) | src_ip[3] as u32);
    sum = sum.wrapping_add(((dst_ip[0] as u32) << 8) | dst_ip[1] as u32);
    sum = sum.wrapping_add(((dst_ip[2] as u32) << 8) | dst_ip[3] as u32);
    sum = sum.wrapping_add(6u32);
    sum = sum.wrapping_add(tcp_len as u32);
    sum = sum.wrapping_add(sum_be16_words(tcp_base, tcp_len));
    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

pub fn send_arp(mmio_base: usize, src_mac: [u8; 6], src_ip: [u8; 4], target_ip: [u8; 4]) -> bool {
    if !tx_wait_ready() {
        return false;
    }
    let hdr_len = VIRTIO_NET_HDR_LEN;
    let frame_len = ETH_HDR_LEN + ARP_PKT_LEN;
    let payload_len = hdr_len + frame_len;
    unsafe {
        let p = core::ptr::addr_of_mut!(TX_BUF) as *mut u8;
        let mut i = 0usize;
        while i < payload_len {
            mmio::store8(p.add(i), 0u8);
            i += 1;
        }
        let frame = p.add(hdr_len);
        for j in 0..6 {
            mmio::store8(frame.add(j), 0xff);
        }
        for j in 0..6 {
            mmio::store8(frame.add(6 + j), src_mac[j]);
        }
        mmio::store8(frame.add(12), 0x08);
        mmio::store8(frame.add(13), 0x06);
        let arp = frame.add(ETH_HDR_LEN);
        write_be16(arp, 0, 1);
        write_be16(arp, 2, 0x0800);
        mmio::store8(arp.add(4), 6);
        mmio::store8(arp.add(5), 4);
        write_be16(arp, 6, 1);
        for j in 0..6 {
            mmio::store8(arp.add(8 + j), src_mac[j]);
        }
        for j in 0..4 {
            mmio::store8(arp.add(14 + j), src_ip[j]);
        }
        for j in 0..6 {
            mmio::store8(arp.add(18 + j), 0);
        }
        for j in 0..4 {
            mmio::store8(arp.add(24 + j), target_ip[j]);
        }
        let d0 = TXQ.desc;
        mmio::store64(core::ptr::addr_of_mut!((*d0).addr), core::ptr::addr_of!(TX_BUF) as u64);
        mmio::store32(core::ptr::addr_of_mut!((*d0).len), payload_len as u32);
        mmio::store16(core::ptr::addr_of_mut!((*d0).flags), 0);
        mmio::store16(core::ptr::addr_of_mut!((*d0).next), 0);
        let cur = mmio::read16(avail_idx_ptr(&TXQ) as usize);
        let slot = (cur as usize) % TXQ.qsize as usize;
        mmio::store16(unsafe { avail_ring_ptr(&TXQ).add(slot) }, 0);
        mmio::store16(avail_idx_ptr(&TXQ), cur.wrapping_add(1));
        mmio::barrier();
        mmio::write32(mmio_base + QUEUE_NOTIFY, 1);
        TX_LAST_SENT = cur.wrapping_add(1);
    }
    true
}

pub fn send_icmp_echo(
    mmio_base: usize,
    src_mac: [u8; 6],
    src_ip: [u8; 4],
    dst_mac: [u8; 6],
    dst_ip: [u8; 4],
) -> bool {
    if !tx_wait_ready() {
        return false;
    }
    let hdr_len = VIRTIO_NET_HDR_LEN;
    let icmp_payload = b"minios";
    let icmp_len = 8 + icmp_payload.len();
    let ip_len = 20 + icmp_len;
    let frame_len = ETH_HDR_LEN + ip_len;
    let payload_len = hdr_len + frame_len;
    unsafe {
        let p = core::ptr::addr_of_mut!(TX_BUF) as *mut u8;
        let mut i = 0usize;
        while i < payload_len {
            mmio::store8(p.add(i), 0u8);
            i += 1;
        }
        let frame = p.add(hdr_len);
        for j in 0..6 {
            mmio::store8(frame.add(j), dst_mac[j]);
            mmio::store8(frame.add(6 + j), src_mac[j]);
        }
        mmio::store8(frame.add(12), 0x08);
        mmio::store8(frame.add(13), 0x00);
        let ip = frame.add(ETH_HDR_LEN);
        mmio::store8(ip.add(0), 0x45);
        mmio::store8(ip.add(1), 0x00);
        write_be16(ip, 2, ip_len as u16);
        write_be16(ip, 4, 0);
        write_be16(ip, 6, 0);
        mmio::store8(ip.add(8), 64);
        mmio::store8(ip.add(9), 1);
        write_be16(ip, 10, 0);
        for j in 0..4 {
            mmio::store8(ip.add(12 + j), src_ip[j]);
            mmio::store8(ip.add(16 + j), dst_ip[j]);
        }
        let ip_csum = checksum16_be(ip as usize, 20);
        write_be16(ip, 10, ip_csum);
        let icmp = unsafe { ip.add(20) };
        mmio::store8(icmp.add(0), 8);
        mmio::store8(icmp.add(1), 0);
        write_be16(icmp, 2, 0);
        write_be16(icmp, 4, 0x1234);
        write_be16(icmp, 6, 1);
        let mut j = 0usize;
        while j < icmp_payload.len() {
            mmio::store8(icmp.add(8 + j), icmp_payload[j]);
            j += 1;
        }
        let icmp_csum = checksum16_be(icmp as usize, icmp_len);
        write_be16(icmp, 2, icmp_csum);
        let d0 = TXQ.desc;
        mmio::store64(core::ptr::addr_of_mut!((*d0).addr), core::ptr::addr_of!(TX_BUF) as u64);
        mmio::store32(core::ptr::addr_of_mut!((*d0).len), payload_len as u32);
        mmio::store16(core::ptr::addr_of_mut!((*d0).flags), 0);
        mmio::store16(core::ptr::addr_of_mut!((*d0).next), 0);
        let cur = mmio::read16(avail_idx_ptr(&TXQ) as usize);
        let slot = (cur as usize) % TXQ.qsize as usize;
        mmio::store16(unsafe { avail_ring_ptr(&TXQ).add(slot) }, 0);
        mmio::store16(avail_idx_ptr(&TXQ), cur.wrapping_add(1));
        mmio::barrier();
        mmio::write32(mmio_base + QUEUE_NOTIFY, 1);
        TX_LAST_SENT = cur.wrapping_add(1);
    }
    true
}

pub fn send_udp(
    mmio_base: usize,
    src_mac: [u8; 6],
    src_ip: [u8; 4],
    src_port: u16,
    dst_mac: [u8; 6],
    dst_ip: [u8; 4],
    dst_port: u16,
    payload: &[u8],
) -> bool {
    if !tx_wait_ready() {
        return false;
    }
    let hdr_len = VIRTIO_NET_HDR_LEN;
    let udp_len = 8 + payload.len();
    let ip_len = 20 + udp_len;
    let frame_len = ETH_HDR_LEN + ip_len;
    let total_len = hdr_len + frame_len;
    if total_len > TX_BUF_LEN {
        uart::write_str("virtio-net udp payload too large\n");
        return false;
    }
    unsafe {
        let p = core::ptr::addr_of_mut!(TX_BUF) as *mut u8;
        let mut i = 0usize;
        while i < total_len {
            mmio::store8(p.add(i), 0u8);
            i += 1;
        }
        let frame = p.add(hdr_len);
        for j in 0..6 {
            mmio::store8(frame.add(j), dst_mac[j]);
            mmio::store8(frame.add(6 + j), src_mac[j]);
        }
        mmio::store8(frame.add(12), 0x08);
        mmio::store8(frame.add(13), 0x00);
        let ip = frame.add(ETH_HDR_LEN);
        mmio::store8(ip.add(0), 0x45);
        mmio::store8(ip.add(1), 0x00);
        write_be16(ip, 2, ip_len as u16);
        write_be16(ip, 4, 0);
        write_be16(ip, 6, 0);
        mmio::store8(ip.add(8), 64);
        mmio::store8(ip.add(9), 17);
        write_be16(ip, 10, 0);
        for j in 0..4 {
            mmio::store8(ip.add(12 + j), src_ip[j]);
            mmio::store8(ip.add(16 + j), dst_ip[j]);
        }
        let ip_csum = checksum16_be(ip as usize, 20);
        write_be16(ip, 10, ip_csum);
        let udp = ip.add(20);
        write_be16(udp, 0, src_port);
        write_be16(udp, 2, dst_port);
        write_be16(udp, 4, udp_len as u16);
        write_be16(udp, 6, 0);
        let mut j = 0usize;
        while j < payload.len() {
            mmio::store8(udp.add(8 + j), payload[j]);
            j += 1;
        }
        let d0 = TXQ.desc;
        mmio::store64(core::ptr::addr_of_mut!((*d0).addr), core::ptr::addr_of!(TX_BUF) as u64);
        mmio::store32(core::ptr::addr_of_mut!((*d0).len), total_len as u32);
        mmio::store16(core::ptr::addr_of_mut!((*d0).flags), 0);
        mmio::store16(core::ptr::addr_of_mut!((*d0).next), 0);
        let cur = mmio::read16(avail_idx_ptr(&TXQ) as usize);
        let slot = (cur as usize) % TXQ.qsize as usize;
        mmio::store16(unsafe { avail_ring_ptr(&TXQ).add(slot) }, 0);
        mmio::store16(avail_idx_ptr(&TXQ), cur.wrapping_add(1));
        mmio::barrier();
        mmio::write32(mmio_base + QUEUE_NOTIFY, 1);
        TX_LAST_SENT = cur.wrapping_add(1);
    }
    true
}

pub fn send_tcp(
    mmio_base: usize,
    src_mac: [u8; 6],
    src_ip: [u8; 4],
    src_port: u16,
    dst_mac: [u8; 6],
    dst_ip: [u8; 4],
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    payload: &[u8],
) -> bool {
    if !tx_wait_ready() {
        return false;
    }
    let hdr_len = VIRTIO_NET_HDR_LEN;
    let tcp_len = 20 + payload.len();
    let ip_len = 20 + tcp_len;
    let frame_len = ETH_HDR_LEN + ip_len;
    let total_len = hdr_len + frame_len;
    if total_len > TX_BUF_LEN {
        uart::write_str("virtio-net tcp payload too large\n");
        return false;
    }
    unsafe {
        let p = core::ptr::addr_of_mut!(TX_BUF) as *mut u8;
        let mut i = 0usize;
        while i < total_len {
            mmio::store8(p.add(i), 0u8);
            i += 1;
        }
        let frame = p.add(hdr_len);
        for j in 0..6 {
            mmio::store8(frame.add(j), dst_mac[j]);
            mmio::store8(frame.add(6 + j), src_mac[j]);
        }
        mmio::store8(frame.add(12), 0x08);
        mmio::store8(frame.add(13), 0x00);
        let ip = frame.add(ETH_HDR_LEN);
        mmio::store8(ip.add(0), 0x45);
        mmio::store8(ip.add(1), 0x00);
        write_be16(ip, 2, ip_len as u16);
        write_be16(ip, 4, 0);
        write_be16(ip, 6, 0);
        mmio::store8(ip.add(8), 64);
        mmio::store8(ip.add(9), 6);
        write_be16(ip, 10, 0);
        for j in 0..4 {
            mmio::store8(ip.add(12 + j), src_ip[j]);
            mmio::store8(ip.add(16 + j), dst_ip[j]);
        }
        let ip_csum = checksum16_be(ip as usize, 20);
        write_be16(ip, 10, ip_csum);
        let tcp = ip.add(20);
        write_be16(tcp, 0, src_port);
        write_be16(tcp, 2, dst_port);
        write_be32(tcp, 4, seq);
        write_be32(tcp, 8, ack);
        mmio::store8(tcp.add(12), (5u8 << 4));
        mmio::store8(tcp.add(13), flags);
        write_be16(tcp, 14, 16384);
        write_be16(tcp, 16, 0);
        write_be16(tcp, 18, 0);
        let mut j = 0usize;
        while j < payload.len() {
            mmio::store8(tcp.add(20 + j), payload[j]);
            j += 1;
        }
        let tcp_csum = checksum16_tcp(src_ip, dst_ip, tcp as usize, tcp_len);
        write_be16(tcp, 16, tcp_csum);
        let d0 = TXQ.desc;
        mmio::store64(core::ptr::addr_of_mut!((*d0).addr), core::ptr::addr_of!(TX_BUF) as u64);
        mmio::store32(core::ptr::addr_of_mut!((*d0).len), total_len as u32);
        mmio::store16(core::ptr::addr_of_mut!((*d0).flags), 0);
        mmio::store16(core::ptr::addr_of_mut!((*d0).next), 0);
        let cur = mmio::read16(avail_idx_ptr(&TXQ) as usize);
        let slot = (cur as usize) % TXQ.qsize as usize;
        mmio::store16(unsafe { avail_ring_ptr(&TXQ).add(slot) }, 0);
        mmio::store16(avail_idx_ptr(&TXQ), cur.wrapping_add(1));
        mmio::barrier();
        mmio::write32(mmio_base + QUEUE_NOTIFY, 1);
        TX_LAST_SENT = cur.wrapping_add(1);
    }
    true
}

pub fn max_tcp_payload_len() -> usize {
    TX_BUF_LEN.saturating_sub(TX_FRAME_OVERHEAD)
}

pub fn parse_rx_arp() -> bool {
    unsafe {
        let (p, _rx_len) = match current_rx_bounds(VIRTIO_NET_HDR_LEN + ETH_HDR_LEN + ARP_PKT_LEN) {
            Some(v) => v,
            None => return false,
        };
        let frame = p + VIRTIO_NET_HDR_LEN;
        let eth_type = ((mmio::read8(frame + 12) as u16) << 8) | (mmio::read8(frame + 13) as u16);
        if eth_type != 0x0806 {
            return false;
        }
        let arp = frame + ETH_HDR_LEN;
        let op = ((mmio::read8(arp + 6) as u16) << 8) | (mmio::read8(arp + 7) as u16);
        if op != 2 {
            return false;
        }
        uart::write_str("virtio-net rx arp reply\n");
        uart::write_str("virtio-net rx arp sender mac: ");
        let mut smac = [0u8; 6];
        for j in 0..6 {
            let b = mmio::read8(arp + 8 + j);
            smac[j] = b;
            uart::write_u64_hex(b as u64);
            if j != 5 {
                uart::write_str(":");
            }
        }
        uart::write_str("\n");
        uart::write_str("virtio-net rx arp sender ip: ");
        let mut sip = [0u8; 4];
        for j in 0..4 {
            let b = mmio::read8(arp + 14 + j);
            sip[j] = b;
            uart::write_u64_hex(b as u64);
            if j != 3 {
                uart::write_str(".");
            }
        }
        uart::write_str("\n");
        LAST_ARP_MAC = smac;
        LAST_ARP_IP = sip;
        LAST_ARP_VALID = true;
        arp_cache_store(smac, sip);
        true
    }
}

pub fn last_arp_peer() -> Option<([u8; 6], [u8; 4])> {
    unsafe {
        if !LAST_ARP_VALID {
            return None;
        }
        Some((LAST_ARP_MAC, LAST_ARP_IP))
    }
}

pub fn parse_rx_udp_any() -> Option<([u8; 4], u16, u16, usize, usize)> {
    unsafe {
        let (p, rx_len) = current_rx_bounds(VIRTIO_NET_HDR_LEN + ETH_HDR_LEN + 20)?;
        let frame = p + VIRTIO_NET_HDR_LEN;
        let eth_type = ((mmio::read8(frame + 12) as u16) << 8) | (mmio::read8(frame + 13) as u16);
        if eth_type != 0x0800 {
            return None;
        }
        let ip = frame + ETH_HDR_LEN;
        let ver_ihl = mmio::read8(ip + 0);
        if (ver_ihl >> 4) != 4 {
            return None;
        }
        let ihl = (ver_ihl & 0x0f) as usize * 4;
        if ihl < 20 {
            return None;
        }
        let ip_total_len = ((mmio::read8(ip + 2) as u16) << 8) | (mmio::read8(ip + 3) as u16);
        let max_ip_len = RX_BUF_LEN.saturating_sub(VIRTIO_NET_HDR_LEN + ETH_HDR_LEN) as u16;
        if ip_total_len < (ihl as u16 + 8) || ip_total_len > max_ip_len {
            return None;
        }
        let proto = mmio::read8(ip + 9);
        if proto != 17 {
            return None;
        }
        let src_ip = [
            mmio::read8(ip + 12),
            mmio::read8(ip + 13),
            mmio::read8(ip + 14),
            mmio::read8(ip + 15),
        ];
        let udp = ip + ihl;
        let src_port = ((mmio::read8(udp + 0) as u16) << 8) | (mmio::read8(udp + 1) as u16);
        let dst_port = ((mmio::read8(udp + 2) as u16) << 8) | (mmio::read8(udp + 3) as u16);
        let udp_len = ((mmio::read8(udp + 4) as u16) << 8) | (mmio::read8(udp + 5) as u16);
        if udp_len < 8 || udp_len > (ip_total_len - ihl as u16) {
            return None;
        }
        let payload_len = (udp_len - 8) as usize;
        let payload_addr = udp + 8;
        let buf_end = p + rx_len;
        let payload_end = match payload_addr.checked_add(payload_len) {
            Some(v) => v,
            None => return None,
        };
        if payload_addr < p || payload_end > buf_end {
            return None;
        }
        Some((src_ip, src_port, dst_port, payload_addr, payload_len))
    }
}

pub fn parse_rx_udp() -> Option<([u8; 4], u16, u16, usize, usize)> {
    unsafe {
        let (p, rx_len) = current_rx_bounds(VIRTIO_NET_HDR_LEN + ETH_HDR_LEN + 20)?;
        let frame = p + VIRTIO_NET_HDR_LEN;
        let eth_type = ((mmio::read8(frame + 12) as u16) << 8) | (mmio::read8(frame + 13) as u16);
        if eth_type != 0x0800 {
            return None;
        }
        let ip = frame + ETH_HDR_LEN;
        let ver_ihl = mmio::read8(ip + 0);
        if (ver_ihl >> 4) != 4 {
            return None;
        }
        let ihl = (ver_ihl & 0x0f) as usize * 4;
        if ihl < 20 {
            return None;
        }
        let ip_total_len = ((mmio::read8(ip + 2) as u16) << 8) | (mmio::read8(ip + 3) as u16);
        let max_ip_len = RX_BUF_LEN.saturating_sub(VIRTIO_NET_HDR_LEN + ETH_HDR_LEN) as u16;
        if ip_total_len < (ihl as u16 + 8) || ip_total_len > max_ip_len {
            return None;
        }
        let proto = mmio::read8(ip + 9);
        if proto != 17 {
            return None;
        }
        let src_ip = [
            mmio::read8(ip + 12),
            mmio::read8(ip + 13),
            mmio::read8(ip + 14),
            mmio::read8(ip + 15),
        ];
        if src_ip == [10, 0, 2, 2] {
            return None;
        }
        let udp = ip + ihl;
        let src_port = ((mmio::read8(udp + 0) as u16) << 8) | (mmio::read8(udp + 1) as u16);
        let dst_port = ((mmio::read8(udp + 2) as u16) << 8) | (mmio::read8(udp + 3) as u16);
        let udp_len = ((mmio::read8(udp + 4) as u16) << 8) | (mmio::read8(udp + 5) as u16);
        if udp_len < 8 || udp_len > (ip_total_len - ihl as u16) {
            return None;
        }
        let payload_len = (udp_len - 8) as usize;
        let payload_addr = udp + 8;
        let buf_end = p + rx_len;
        let payload_end = match payload_addr.checked_add(payload_len) {
            Some(v) => v,
            None => return None,
        };
        if payload_addr < p || payload_end > buf_end {
            return None;
        }
        Some((src_ip, src_port, dst_port, payload_addr, payload_len))
    }
}

pub fn parse_rx_tcp() -> Option<([u8; 4], u16, u16, u32, u32, u8, usize, usize)> {
    unsafe {
        let (p, rx_len) = current_rx_bounds(VIRTIO_NET_HDR_LEN + ETH_HDR_LEN + 20)?;
        let frame = p + VIRTIO_NET_HDR_LEN;
        let eth_type = ((mmio::read8(frame + 12) as u16) << 8) | (mmio::read8(frame + 13) as u16);
        if eth_type != 0x0800 {
            return None;
        }
        let ip = frame + ETH_HDR_LEN;
        let ver_ihl = mmio::read8(ip + 0);
        if (ver_ihl >> 4) != 4 {
            return None;
        }
        let ihl = (ver_ihl & 0x0f) as usize * 4;
        if ihl < 20 {
            return None;
        }
        let ip_total_len = ((mmio::read8(ip + 2) as u16) << 8) | (mmio::read8(ip + 3) as u16);
        let max_ip_len = RX_BUF_LEN.saturating_sub(VIRTIO_NET_HDR_LEN + ETH_HDR_LEN) as u16;
        if ip_total_len < (ihl as u16 + 20) || ip_total_len > max_ip_len {
            return None;
        }
        let proto = mmio::read8(ip + 9);
        if proto != 6 {
            return None;
        }
        let src_ip = [
            mmio::read8(ip + 12),
            mmio::read8(ip + 13),
            mmio::read8(ip + 14),
            mmio::read8(ip + 15),
        ];
        let tcp = ip + ihl;
        let src_port = ((mmio::read8(tcp + 0) as u16) << 8) | (mmio::read8(tcp + 1) as u16);
        let dst_port = ((mmio::read8(tcp + 2) as u16) << 8) | (mmio::read8(tcp + 3) as u16);
        let seq = (mmio::read8(tcp + 4) as u32) << 24
            | (mmio::read8(tcp + 5) as u32) << 16
            | (mmio::read8(tcp + 6) as u32) << 8
            | (mmio::read8(tcp + 7) as u32);
        let ack = (mmio::read8(tcp + 8) as u32) << 24
            | (mmio::read8(tcp + 9) as u32) << 16
            | (mmio::read8(tcp + 10) as u32) << 8
            | (mmio::read8(tcp + 11) as u32);
        let data_off = (mmio::read8(tcp + 12) >> 4) as usize * 4;
        if data_off < 20 {
            return None;
        }
        let flags = mmio::read8(tcp + 13);
        let tcp_len = (ip_total_len as usize).saturating_sub(ihl);
        if tcp_len < data_off {
            return None;
        }
        let payload_len = tcp_len - data_off;
        let payload_addr = tcp + data_off;
        let buf_end = p + rx_len;
        let payload_end = match payload_addr.checked_add(payload_len) {
            Some(v) => v,
            None => return None,
        };
        if payload_addr < p || payload_end > buf_end {
            return None;
        }
        Some((src_ip, src_port, dst_port, seq, ack, flags, payload_addr, payload_len))
    }
}

pub fn rx_eth_type() -> u16 {
    unsafe {
        let (p, _rx_len) = match current_rx_bounds(VIRTIO_NET_HDR_LEN + ETH_HDR_LEN) {
            Some(v) => v,
            None => return 0,
        };
        if p == 0 {
            return 0;
        }
        let frame = p + VIRTIO_NET_HDR_LEN;
        ((mmio::read8(frame + 12) as u16) << 8) | (mmio::read8(frame + 13) as u16)
    }
}

pub fn rx_ip_proto() -> Option<u8> {
    unsafe {
        if current_rx_bounds(VIRTIO_NET_HDR_LEN + ETH_HDR_LEN + 20).is_none() {
            return None;
        }
        let eth_type = rx_eth_type();
        if eth_type != 0x0800 {
            return None;
        }
        let p = RX_BUF_PTR as usize;
        let ip = p + VIRTIO_NET_HDR_LEN + ETH_HDR_LEN;
        let ver_ihl = mmio::read8(ip + 0);
        if (ver_ihl >> 4) != 4 {
            return None;
        }
        Some(mmio::read8(ip + 9))
    }
}

pub fn rx_ip_src() -> Option<[u8; 4]> {
    unsafe {
        if current_rx_bounds(VIRTIO_NET_HDR_LEN + ETH_HDR_LEN + 20).is_none() {
            return None;
        }
        let eth_type = rx_eth_type();
        if eth_type != 0x0800 {
            return None;
        }
        let p = RX_BUF_PTR as usize;
        let ip = p + VIRTIO_NET_HDR_LEN + ETH_HDR_LEN;
        let ver_ihl = mmio::read8(ip + 0);
        if (ver_ihl >> 4) != 4 {
            return None;
        }
        Some([
            mmio::read8(ip + 12),
            mmio::read8(ip + 13),
            mmio::read8(ip + 14),
            mmio::read8(ip + 15),
        ])
    }
}

pub fn rx_payload_eq(addr: usize, len: usize, pat: &[u8]) -> bool {
    if pat.len() != len {
        return false;
    }
    let mut i = 0usize;
    while i < len {
        let b = unsafe { mmio::read8(addr + i) };
        if b != pat[i] {
            return false;
        }
        i += 1;
    }
    true
}

pub fn rx_copy(addr: usize, len: usize, dst: &mut [u8]) -> usize {
    let n = if len < dst.len() { len } else { dst.len() };
    let mut i = 0usize;
    while i < n {
        dst[i] = unsafe { mmio::read8(addr + i) };
        i += 1;
    }
    n
}

pub fn parse_rx_icmp_reply() -> bool {
    unsafe {
        let (p, _rx_len) = match current_rx_bounds(VIRTIO_NET_HDR_LEN + ETH_HDR_LEN + 20) {
            Some(v) => v,
            None => return false,
        };
        let frame = p + VIRTIO_NET_HDR_LEN;
        let eth_type = ((mmio::read8(frame + 12) as u16) << 8) | (mmio::read8(frame + 13) as u16);
        if eth_type != 0x0800 {
            return false;
        }
        let ip = frame + ETH_HDR_LEN;
        let ver_ihl = mmio::read8(ip + 0);
        let ihl = (ver_ihl & 0x0f) as usize * 4;
        let proto = mmio::read8(ip + 9);
        if proto != 1 || ihl < 20 {
            return false;
        }
        let icmp = ip + ihl;
        let icmp_type = mmio::read8(icmp + 0);
        let icmp_code = mmio::read8(icmp + 1);
        if icmp_type != 0 || icmp_code != 0 {
            return false;
        }
        let id = ((mmio::read8(icmp + 4) as u16) << 8) | (mmio::read8(icmp + 5) as u16);
        let seq = ((mmio::read8(icmp + 6) as u16) << 8) | (mmio::read8(icmp + 7) as u16);
        uart::write_str("virtio-net rx icmp echo reply\n");
        uart::write_str("virtio-net rx icmp id: 0x");
        uart::write_u64_hex(id as u64);
        uart::write_str(" seq: 0x");
        uart::write_u64_hex(seq as u64);
        uart::write_str("\n");
        uart::write_str("virtio-net rx icmp src ip: ");
        for j in 0..4 {
            let b = mmio::read8(ip + 12 + j);
            uart::write_u64_hex(b as u64);
            if j != 3 {
                uart::write_str(".");
            }
        }
        uart::write_str("\n");
        true
    }
}

pub fn rx_rearm(mmio_base: usize) {
    unsafe {
        if RXQ.qsize == 0 || !RX_CUR_VALID || !RX_NEEDS_REARM {
            return;
        }
        rx_rearm_id(mmio_base, RX_CUR_ID);
        RX_NEEDS_REARM = false;
    }
}

pub fn rx_rearm_id(mmio_base: usize, id: u16) {
    unsafe {
        if RXQ.qsize == 0 {
            return;
        }
        let cur = mmio::read16(avail_idx_ptr(&RXQ) as usize);
        let slot = (cur as usize) % RXQ.qsize as usize;
        mmio::store16(avail_ring_ptr(&RXQ).add(slot), id);
        mmio::store16(avail_idx_ptr(&RXQ), cur.wrapping_add(1));
        mmio::barrier();
        mmio::write32(mmio_base + QUEUE_NOTIFY, 0);
    }
}

pub fn tx_used_idx() -> u16 {
    mmio::barrier();
    unsafe { mmio::read16(used_idx_ptr(&TXQ) as usize) }
}

pub fn rx_used_idx() -> u16 {
    mmio::barrier();
    unsafe { mmio::read16(used_idx_ptr(&RXQ) as usize) }
}

pub fn rx_last_used(idx: u16) -> Option<VirtqUsedElem> {
    if idx == 0 {
        return None;
    }
    let qsize = unsafe { RXQ.qsize as usize };
    if qsize == 0 {
        return None;
    }
    let slot = (idx.wrapping_sub(1) as usize) % qsize;
    let used_base = unsafe { RXQ.used } as *const u16;
    let elem_base = unsafe { used_base.add(2) as *const VirtqUsedElem };
    let elem_ptr = unsafe { elem_base.add(slot) };
    let addr = elem_ptr as usize;
    let id = unsafe { mmio::read32(addr) };
    let len = unsafe { mmio::read32(addr + 4) };
    let elem = VirtqUsedElem { id, len };
    Some(elem)
}

pub fn rx_used_elem_fields(idx: u16) -> Option<(u32, u32)> {
    if idx == 0 {
        return None;
    }
    let qsize = unsafe { RXQ.qsize as usize };
    if qsize == 0 || unsafe { RXQ.used }.is_null() {
        return None;
    }
    let slot = (idx.wrapping_sub(1) as usize) % qsize;
    let used_base = unsafe { RXQ.used } as *const u16;
    let elem_base = unsafe { used_base.add(2) as *const u32 };
    let elem_ptr = unsafe { elem_base.add(slot * 2) } as usize;
    let id = unsafe { mmio::read32(elem_ptr) };
    let len = unsafe { mmio::read32(elem_ptr + 4) };
    Some((id, len))
}

pub fn rx_buf_first_byte() -> u8 {
    unsafe {
        if RX_BUF_PTR.is_null() {
            return 0;
        }
        mmio::read8(RX_BUF_PTR as usize)
    }
}

pub fn rx_buf_addr() -> usize {
    unsafe { RX_BUF_PTR as usize }
}

pub fn rx_set_current(id: u32, len: u32) -> bool {
    unsafe {
        let idx = id as usize;
        if idx >= RX_BUF_COUNT {
            RX_CUR_VALID = false;
            RX_CUR_LEN = 0;
            RX_BUF_PTR = core::ptr::null_mut();
            return false;
        }
        RX_BUF_PTR = RX_BUF_PTRS[idx];
        RX_CUR_ID = idx as u16;
        RX_CUR_LEN = if (len as usize) > RX_BUF_LEN {
            RX_BUF_LEN
        } else {
            len as usize
        };
        RX_CUR_VALID = true;
        RX_NEEDS_REARM = true;
        true
    }
}

pub fn dump_rx_bytes(count: usize) {
    let n = if count > RX_BUF_LEN { RX_BUF_LEN } else { count };
    unsafe {
        let p = RX_BUF_PTR as usize;
        let mut i = 0usize;
        while i < n {
            let b = mmio::read8(p + i);
            uart::write_u64_hex(b as u64);
            uart::write_str(" ");
            i += 1;
        }
        uart::write_str("\n");
    }
}

pub fn read_mac(base: usize) -> [u8; MAC_LEN] {
    let mut mac = [0u8; MAC_LEN];
    let w0 = unsafe { mmio::read32(base + NET_CONFIG_OFFSET) };
    let w1 = unsafe { mmio::read32(base + NET_CONFIG_OFFSET + 4) };
    let bytes = [
        (w0 & 0xff) as u8,
        ((w0 >> 8) & 0xff) as u8,
        ((w0 >> 16) & 0xff) as u8,
        ((w0 >> 24) & 0xff) as u8,
        (w1 & 0xff) as u8,
        ((w1 >> 8) & 0xff) as u8,
    ];
    mac.copy_from_slice(&bytes);
    mac
}

pub fn reset_status(base: usize) {
    unsafe { mmio::write32(base + MMIO_STATUS, 0) };
}

pub fn set_status(base: usize, status: u32) {
    unsafe { mmio::write32(base + MMIO_STATUS, status) };
}

pub fn dump_status(base: usize, label: &str) {
    let status = unsafe { mmio::read32(base + MMIO_STATUS) };
    uart::write_str(label);
    uart::write_str(" status: 0x");
    uart::write_u64_hex(status as u64);
    uart::write_str("\n");
}

pub fn dump_queue(base: usize, qidx: u32, modern: bool) {
    unsafe { mmio::write32(base + MMIO_QUEUE_SEL, qidx) };
    let max = unsafe { mmio::read32(base + MMIO_QUEUE_NUM_MAX) };
    let num = unsafe { mmio::read32(base + MMIO_QUEUE_NUM) };
    let align = unsafe { mmio::read32(base + MMIO_QUEUE_ALIGN) };
    let pfn = unsafe { mmio::read32(base + MMIO_QUEUE_PFN) };
    let ready = unsafe {
        if modern {
            mmio::read32(base + MMIO_QUEUE_READY_MODERN)
        } else {
            (mmio::read32(base + MMIO_QUEUE_PFN) != 0) as u32
        }
    };
    uart::write_str("virtio-net q");
    uart::write_u64_hex(qidx as u64);
    uart::write_str(" max: 0x");
    uart::write_u64_hex(max as u64);
    uart::write_str(" num: 0x");
    uart::write_u64_hex(num as u64);
    uart::write_str(" align: 0x");
    uart::write_u64_hex(align as u64);
    uart::write_str(" pfn: 0x");
    uart::write_u64_hex(pfn as u64);
    uart::write_str(" ready: 0x");
    uart::write_u64_hex(ready as u64);
    uart::write_str("\n");
    if modern {
        let desc_lo = unsafe { mmio::read32(base + MMIO_QUEUE_DESC_LOW) };
        let desc_hi = unsafe { mmio::read32(base + MMIO_QUEUE_DESC_HIGH) };
        let avail_lo = unsafe { mmio::read32(base + MMIO_QUEUE_AVAIL_LOW) };
        let avail_hi = unsafe { mmio::read32(base + MMIO_QUEUE_AVAIL_HIGH) };
        let used_lo = unsafe { mmio::read32(base + MMIO_QUEUE_USED_LOW) };
        let used_hi = unsafe { mmio::read32(base + MMIO_QUEUE_USED_HIGH) };
        uart::write_str("virtio-net q");
        uart::write_u64_hex(qidx as u64);
        uart::write_str(" desc: 0x");
        uart::write_u64_hex(((desc_hi as u64) << 32) | desc_lo as u64);
        uart::write_str(" avail: 0x");
        uart::write_u64_hex(((avail_hi as u64) << 32) | avail_lo as u64);
        uart::write_str(" used: 0x");
        uart::write_u64_hex(((used_hi as u64) << 32) | used_lo as u64);
        uart::write_str("\n");
    }
}

fn dump_mmio_state(base: usize, label: &str) {
    let magic = unsafe { mmio::read32(base + MMIO_MAGIC_VALUE) };
    let ver = unsafe { mmio::read32(base + virtio::MMIO_VERSION) };
    let status = unsafe { mmio::read32(base + MMIO_STATUS) };
    let isr = unsafe { mmio::read32(base + MMIO_INTERRUPT_STATUS) };
    uart::write_str(label);
    uart::write_str(" magic: 0x");
    uart::write_u64_hex(magic as u64);
    uart::write_str(" ver: 0x");
    uart::write_u64_hex(ver as u64);
    uart::write_str(" status: 0x");
    uart::write_u64_hex(status as u64);
    uart::write_str(" isr: 0x");
    uart::write_u64_hex(isr as u64);
    uart::write_str("\n");
}

fn dump_queue_regs(base: usize, qidx: u32, modern: bool, label: &str) {
    unsafe { mmio::write32(base + MMIO_QUEUE_SEL, qidx) };
    let num = unsafe { mmio::read32(base + MMIO_QUEUE_NUM) };
    let align = unsafe { mmio::read32(base + MMIO_QUEUE_ALIGN) };
    let pfn = unsafe { mmio::read32(base + MMIO_QUEUE_PFN) };
    let ready = unsafe {
        if modern {
            mmio::read32(base + MMIO_QUEUE_READY_MODERN)
        } else {
            (mmio::read32(base + MMIO_QUEUE_PFN) != 0) as u32
        }
    };
    uart::write_str(label);
    uart::write_str(" q");
    uart::write_u64_hex(qidx as u64);
    uart::write_str(" num: 0x");
    uart::write_u64_hex(num as u64);
    uart::write_str(" align: 0x");
    uart::write_u64_hex(align as u64);
    uart::write_str(" pfn: 0x");
    uart::write_u64_hex(pfn as u64);
    uart::write_str(" ready: 0x");
    uart::write_u64_hex(ready as u64);
    uart::write_str("\n");
}

fn dump_queue_addrs(base: usize, qidx: u32, label: &str) {
    unsafe { mmio::write32(base + MMIO_QUEUE_SEL, qidx) };
    let desc_lo = unsafe { mmio::read32(base + MMIO_QUEUE_DESC_LOW) };
    let desc_hi = unsafe { mmio::read32(base + MMIO_QUEUE_DESC_HIGH) };
    let avail_lo = unsafe { mmio::read32(base + MMIO_QUEUE_AVAIL_LOW) };
    let avail_hi = unsafe { mmio::read32(base + MMIO_QUEUE_AVAIL_HIGH) };
    let used_lo = unsafe { mmio::read32(base + MMIO_QUEUE_USED_LOW) };
    let used_hi = unsafe { mmio::read32(base + MMIO_QUEUE_USED_HIGH) };
    uart::write_str(label);
    uart::write_str(" q");
    uart::write_u64_hex(qidx as u64);
    uart::write_str(" desc: 0x");
    uart::write_u64_hex(((desc_hi as u64) << 32) | desc_lo as u64);
    uart::write_str(" avail: 0x");
    uart::write_u64_hex(((avail_hi as u64) << 32) | avail_lo as u64);
    uart::write_str(" used: 0x");
    uart::write_u64_hex(((used_hi as u64) << 32) | used_lo as u64);
    uart::write_str("\n");
}

fn dump_queue_sel(base: usize, qidx: u32, label: &str) {
    let sel = unsafe { mmio::read32(base + MMIO_QUEUE_SEL) };
    uart::write_str(label);
    uart::write_str(" q");
    uart::write_u64_hex(qidx as u64);
    uart::write_str(" sel: 0x");
    uart::write_u64_hex(sel as u64);
    uart::write_str("\n");
}

fn dump_queue_num(base: usize, qidx: u32, label: &str) {
    unsafe { mmio::write32(base + MMIO_QUEUE_SEL, qidx) };
    let num = unsafe { mmio::read32(base + MMIO_QUEUE_NUM) };
    uart::write_str(label);
    uart::write_str(" q");
    uart::write_u64_hex(qidx as u64);
    uart::write_str(" num: 0x");
    uart::write_u64_hex(num as u64);
    uart::write_str("\n");
}

fn dump_queue_ready(base: usize, qidx: u32, modern: bool, label: &str) {
    unsafe { mmio::write32(base + MMIO_QUEUE_SEL, qidx) };
    let ready = unsafe {
        if modern {
            mmio::read32(base + MMIO_QUEUE_READY_MODERN)
        } else {
            (mmio::read32(base + MMIO_QUEUE_PFN) != 0) as u32
        }
    };
    uart::write_str(label);
    uart::write_str(" q");
    uart::write_u64_hex(qidx as u64);
    uart::write_str(" ready: 0x");
    uart::write_u64_hex(ready as u64);
    uart::write_str("\n");
}
