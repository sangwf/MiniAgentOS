#[derive(Clone, Copy)]
pub struct VirtioMmioInfo {
    pub base: u64,
    pub size: u64,
}

const FDT_MAGIC: u32 = 0xd00d_feed;
const FDT_BEGIN_NODE: u32 = 1;
const FDT_END_NODE: u32 = 2;
const FDT_PROP: u32 = 3;
const FDT_NOP: u32 = 4;
const FDT_END: u32 = 9;

pub fn find_first_virtio_mmio(dtb_addr: usize) -> Option<VirtioMmioInfo> {
    let hdr = unsafe { &*(dtb_addr as *const FdtHeader) };
    if be32(hdr.magic) != FDT_MAGIC {
        return None;
    }
    let struct_off = be32(hdr.off_dt_struct) as usize;
    let strings_off = be32(hdr.off_dt_strings) as usize;
    let struct_size = be32(hdr.size_dt_struct) as usize;

    let struct_ptr = (dtb_addr + struct_off) as *const u8;
    let strings_ptr = (dtb_addr + strings_off) as *const u8;

    let mut p = struct_ptr as *const u32;
    let end = (struct_ptr as usize + struct_size) as *const u32;

    let mut node_virtio = false;
    let mut node_reg: Option<(u64, u64)> = None;

    while p < end {
        let token = unsafe { be32(*p) };
        p = unsafe { p.add(1) };
        match token {
            FDT_BEGIN_NODE => {
                // Skip node name string (null-terminated), then align to 4 bytes.
                let mut s = p as *const u8;
                unsafe {
                    while *s != 0 {
                        s = s.add(1);
                    }
                    s = s.add(1);
                }
                p = align4(s as usize) as *const u32;
                node_virtio = false;
                node_reg = None;
            }
            FDT_END_NODE => {
                node_virtio = false;
                node_reg = None;
            }
            FDT_PROP => {
                let len = unsafe { be32(*p) } as usize;
                p = unsafe { p.add(1) };
                let name_off = unsafe { be32(*p) } as usize;
                p = unsafe { p.add(1) };
                let prop_data = p as *const u8;
                let prop_name = unsafe { cstr_at(strings_ptr.add(name_off)) };

                if prop_name == "compatible" {
                    if contains_str(prop_data, len, "virtio,mmio") {
                        node_virtio = true;
                    }
                } else if prop_name == "reg" {
                    if len >= 16 {
                        let cells = prop_data as *const u32;
                        let addr = (be32(unsafe { *cells }) as u64) << 32
                            | be32(unsafe { *cells.add(1) }) as u64;
                        let size = (be32(unsafe { *cells.add(2) }) as u64) << 32
                            | be32(unsafe { *cells.add(3) }) as u64;
                        node_reg = Some((addr, size));
                    }
                }

                // Move to next token, 4-byte aligned.
                let next = (prop_data as usize + len + 3) & !3;
                p = next as *const u32;
                if node_virtio {
                    if let Some((base, size)) = node_reg {
                        return Some(VirtioMmioInfo { base, size });
                    }
                }
            }
            FDT_NOP => {}
            FDT_END => break,
            _ => break,
        }
    }
    None
}

#[repr(C)]
struct FdtHeader {
    magic: u32,
    totalsize: u32,
    off_dt_struct: u32,
    off_dt_strings: u32,
    off_mem_rsvmap: u32,
    version: u32,
    last_comp_version: u32,
    boot_cpuid_phys: u32,
    size_dt_strings: u32,
    size_dt_struct: u32,
}

fn be32(v: u32) -> u32 {
    u32::from_be(v)
}

fn align4(addr: usize) -> usize {
    (addr + 3) & !3
}

unsafe fn cstr_at(mut p: *const u8) -> &'static str {
    let start = p;
    while *p != 0 {
        p = p.add(1);
    }
    let len = p as usize - start as usize;
    core::str::from_utf8_unchecked(core::slice::from_raw_parts(start, len))
}

fn contains_str(p: *const u8, len: usize, needle: &str) -> bool {
    let data = unsafe { core::slice::from_raw_parts(p, len) };
    let mut i = 0usize;
    while i < data.len() {
        let end = data[i..].iter().position(|&b| b == 0).map(|n| i + n).unwrap_or(data.len());
        if let Ok(s) = core::str::from_utf8(&data[i..end]) {
            if s == needle {
                return true;
            }
        }
        i = end + 1;
    }
    false
}
