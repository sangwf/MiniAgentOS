use crate::mmio;

pub struct MmioDevice {
    pub base: usize,
}

impl MmioDevice {
    pub fn new(base: usize) -> Self {
        Self { base }
    }

    pub fn read(&self, off: usize) -> u32 {
        unsafe { mmio::read32(self.base + off) }
    }

    pub fn write(&self, off: usize, val: u32) {
        unsafe { mmio::write32(self.base + off, val) }
    }
}

pub const MMIO_MAGIC_VALUE: usize = 0x000;
pub const MMIO_VERSION: usize = 0x004;
pub const MMIO_DEVICE_ID: usize = 0x008;
pub const MMIO_VENDOR_ID: usize = 0x00c;
pub const MMIO_DEVICE_FEATURES: usize = 0x010;
pub const MMIO_DEVICE_FEATURES_SEL: usize = 0x014;
pub const MMIO_DRIVER_FEATURES: usize = 0x020;
pub const MMIO_DRIVER_FEATURES_SEL: usize = 0x024;
pub const MMIO_STATUS: usize = 0x070;

pub const VIRTIO_MAGIC: u32 = 0x7472_6976;
pub const VIRTIO_DEV_NET: u32 = 1;
pub const VIRTIO_F_VERSION_1: u64 = 1 << 32;

pub const STATUS_ACK: u32 = 1;
pub const STATUS_DRIVER: u32 = 2;
pub const STATUS_DRIVER_OK: u32 = 4;
pub const STATUS_FEATURES_OK: u32 = 8;
pub const STATUS_FAILED: u32 = 0x80;

pub fn probe_mmio(base: usize) -> Option<u32> {
    let dev = MmioDevice::new(base);
    if dev.read(MMIO_MAGIC_VALUE) != VIRTIO_MAGIC {
        return None;
    }
    Some(dev.read(MMIO_DEVICE_ID))
}

pub fn read_device_features(base: usize, modern: bool) -> u64 {
    let dev = MmioDevice::new(base);
    if modern {
        dev.write(MMIO_DEVICE_FEATURES_SEL, 0);
        let lo = dev.read(MMIO_DEVICE_FEATURES) as u64;
        dev.write(MMIO_DEVICE_FEATURES_SEL, 1);
        let hi = dev.read(MMIO_DEVICE_FEATURES) as u64;
        (hi << 32) | lo
    } else {
        dev.read(MMIO_DEVICE_FEATURES) as u64
    }
}

pub fn write_driver_features(base: usize, feats: u64, modern: bool) {
    let dev = MmioDevice::new(base);
    if modern {
        dev.write(MMIO_DRIVER_FEATURES_SEL, 0);
        dev.write(MMIO_DRIVER_FEATURES, feats as u32);
        dev.write(MMIO_DRIVER_FEATURES_SEL, 1);
        dev.write(MMIO_DRIVER_FEATURES, (feats >> 32) as u32);
    } else {
        dev.write(MMIO_DRIVER_FEATURES, feats as u32);
    }
}
