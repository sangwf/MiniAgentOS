use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicUsize, Ordering};

const HEAP_SIZE: usize = 1024 * 1024; // 1 MiB

#[allow(dead_code)]
#[repr(align(16))]
struct HeapSpace([u8; HEAP_SIZE]);

static mut HEAP_SPACE: HeapSpace = HeapSpace([0u8; HEAP_SIZE]);

pub struct BumpAllocator {
    next: AtomicUsize,
}

impl BumpAllocator {
    pub const fn new() -> Self {
        Self {
            next: AtomicUsize::new(0),
        }
    }

    fn heap_bounds(&self) -> (usize, usize) {
        let start = core::ptr::addr_of!(HEAP_SPACE) as usize;
        (start, start + HEAP_SIZE)
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let (heap_start, heap_end) = self.heap_bounds();
        let mut current = self.next.load(Ordering::Relaxed);

        if current == 0 {
            self.next.store(heap_start, Ordering::Relaxed);
            current = heap_start;
        }

        loop {
            let alloc_start = align_up(current, layout.align());
            let alloc_end = match alloc_start.checked_add(layout.size()) {
                Some(end) => end,
                None => return null_mut(),
            };

            if alloc_end > heap_end {
                return null_mut();
            }

            match self.next.compare_exchange(
                current,
                alloc_end,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return alloc_start as *mut u8,
                Err(next) => current = next,
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator does not support deallocation.
    }
}

#[inline(always)]
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    loop {
        unsafe { core::arch::asm!("wfe"); }
    }
}
