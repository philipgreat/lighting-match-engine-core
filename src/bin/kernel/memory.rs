use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicUsize, Ordering};

const HEAP_SIZE: usize = 128 * 1024;

#[repr(align(16))]
struct Heap([u8; HEAP_SIZE]);

static HEAP: Heap = Heap([0; HEAP_SIZE]);
static NEXT: AtomicUsize = AtomicUsize::new(0);

pub struct BumpAllocator;

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator;

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align_mask = layout.align().saturating_sub(1);
        let size = layout.size();

        let result = NEXT.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
            let start = (current + align_mask) & !align_mask;
            let end = start.checked_add(size)?;
            (end <= HEAP_SIZE).then_some(end)
        });

        match result {
            Ok(start) => HEAP.0.as_ptr().wrapping_add(start) as *mut u8,
            Err(_) => null_mut(),
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

pub fn init_heap() {
    NEXT.store(0, Ordering::SeqCst);
}
