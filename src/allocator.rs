use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use spin::Mutex;
use x86_64::{
    structures::paging::{mapper::MapToError, Page, PageTableFlags, Size4KiB, Mapper, FrameAllocator},
    VirtAddr,
};

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE:  usize = 128 * 1024; // 128 KiB

#[global_allocator]
static ALLOCATOR: Locked<LinkedListAllocator> = Locked::new(LinkedListAllocator::new());

pub struct Locked<A> {
    inner: Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked { inner: Mutex::new(inner) }
    }
}

struct ListNode {
    size: usize,
    next: *mut ListNode,
}

struct LinkedListAllocator {
    head: ListNode,
}

// Single-core kernel; raw pointers are only touched under the spin lock.
unsafe impl Send for LinkedListAllocator {}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        LinkedListAllocator { head: ListNode { size: 0, next: null_mut() } }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        let node_ptr = heap_start as *mut ListNode;
        node_ptr.write(ListNode { size: heap_size, next: null_mut() });
        self.head.next = node_ptr;
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut alloc = self.inner.lock();

        // Round up size/align so we can always write a ListNode back on dealloc.
        let size  = layout.size() .max(core::mem::size_of::<ListNode>());
        let align = layout.align().max(core::mem::align_of::<ListNode>());

        let mut current = &mut alloc.head as *mut ListNode;

        while !(*current).next.is_null() {
            let next        = (*current).next;
            let block_start = next as usize;
            let block_end   = block_start + (*next).size;

            let alloc_start = align_up(block_start, align);
            let alloc_end   = alloc_start + size;

            if alloc_end > block_end {
                current = next;
                continue;
            }

            // ── TAIL: free bytes after the allocation ───────────────────────
            // Split them off into a new free node when large enough.
            let tail_size = block_end - alloc_end;
            if tail_size >= core::mem::size_of::<ListNode>() {
                let tail = alloc_end as *mut ListNode;
                tail.write(ListNode { size: tail_size, next: (*next).next });
                (*current).next = tail;
            } else {
                // Tail too small for a node header — include it in this
                // allocation (slight overallocation, avoids stranded bytes).
                (*current).next = (*next).next;
            }

            // ── HEAD: alignment-padding bytes before the allocation ─────────
            // Previously these bytes were silently discarded.  Now we thread
            // them back into the free list when they fit a node header.
            let head_size = alloc_start - block_start;
            if head_size >= core::mem::size_of::<ListNode>() {
                // (*current).next already points at the tail (or the node
                // after the old block if there was no tail).  The head node
                // sits right where `next` used to be and points forward.
                let head = block_start as *mut ListNode;
                head.write(ListNode { size: head_size, next: (*current).next });
                (*current).next = head;
            }
            // (If head_size < sizeof(ListNode) those few padding bytes are
            // unavoidably wasted, but this is a negligible amount.)

            return alloc_start as *mut u8;
        }

        null_mut() // out of memory
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut alloc = self.inner.lock();
        let size = layout.size().max(core::mem::size_of::<ListNode>());

        // Prepend the freed block to the head of the free list.
        // A coalescing pass would reduce fragmentation further, but for a
        // 128 KiB kernel heap this simple policy is sufficient.
        let node = ptr as *mut ListNode;
        node.write(ListNode { size, next: alloc.head.next });
        alloc.head.next = node;
    }
}

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start      = VirtAddr::new(HEAP_START as u64);
        let heap_end        = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page   = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe { mapper.map_to(page, frame, flags, frame_allocator)?.flush(); }
    }

    unsafe { ALLOCATOR.inner.lock().init(HEAP_START, HEAP_SIZE); }

    Ok(())
}