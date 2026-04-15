use core::alloc::{GlobalAlloc, Layout};
use core::ptr::{self, NonNull};
use crate::sync::spinlock::SpinLock;
use crate::memory::physical::{alloc_pages, free_pages, PageFrame};
use core::sync::atomic::{AtomicUsize, AtomicBool, Ordering};

struct FreeBlock {
    next: Option<&'static mut FreeBlock>,
    size: usize,
    is_free: bool,
}

#[repr(C)]
struct BlockHeader {
    magic: u32,
    size: usize,
    is_free: bool,
    checksum: u32,
}

const BLOCK_MAGIC: u32 = 0xDEADBEEF;
const MAX_FREE_LISTS: usize = 32;

struct FreeLists {
    heads: [Option<&'static mut FreeBlock>; MAX_FREE_LISTS],
}

impl FreeLists {
    fn new() -> Self {
        FreeLists { heads: [None; MAX_FREE_LISTS] }
    }

    unsafe fn get_list_index(&self, size: usize) -> Option<usize> {
        if size == 0 { return None; }
        let log2_size = 63 - size.leading_zeros() as usize;
        let index = log2_size.min(MAX_FREE_LISTS - 1);
        Some(index)
    }

    unsafe fn insert(&mut self, ptr: *mut u8, size: usize) {
        if let Some(index) = self.get_list_index(size) {
            let block = ptr as *mut FreeBlock;
            (*block).next = self.heads[index].take();
            (*block).size = size;
            (*block).is_free = true;
            self.heads[index] = Some(&mut *block);
        }
    }

    unsafe fn remove_best_fit(&mut self, size: usize, align: usize) -> Option<*mut u8> {
        let target_index = self.get_list_index(size).unwrap_or(0);
        for i in target_index..MAX_FREE_LISTS {
            let mut prev = &mut self.heads[i];
            while let Some(curr) = prev.as_deref_mut() {
                let addr = curr as *const _ as usize;
                let aligned_addr = (addr + align - 1) & !(align - 1);
                let offset = aligned_addr - addr;
                
                if offset + size <= curr.size {
                    let old = prev.take();
                    *prev = old.unwrap().next.take();
                    
                    if curr.size > size + 32 {
                        let remaining_size = curr.size - size;
                        if remaining_size >= 32 {
                            let new_block_ptr = (curr as *mut FreeBlock).add(size) as *mut u8;
                            self.insert(new_block_ptr, remaining_size);
                        }
                    }
                    
                    return Some((curr as *mut FreeBlock) as *mut u8);
                }
                prev = &mut curr.next;
            }
        }
        None
    }
}

pub struct Heap {
    free_lists: SpinLock<FreeLists>,
    total_bytes: AtomicUsize,
    used_bytes: AtomicUsize,
    pages_allocated: AtomicUsize,
    initialized: AtomicBool,
}

impl Heap {
    pub const fn new() -> Self {
        Heap {
            free_lists: SpinLock::new(FreeLists::new()),
            total_bytes: AtomicUsize::new(0),
            used_bytes: AtomicUsize::new(0),
            pages_allocated: AtomicUsize::new(0),
            initialized: AtomicBool::new(false),
        }
    }

    pub unsafe fn init(&mut self, start: usize, size: usize) {
        self.free_lists.lock().insert(start as *mut u8, size);
        self.total_bytes.store(size, Ordering::Relaxed);
        self.initialized.store(true, Ordering::Relaxed);
    }

    unsafe fn alloc_pages_for_heap(&self, size: usize) -> Option<*mut u8> {
        let num_pages = (size + 4095) >> 12;
        if let Some(frames) = alloc_pages(num_pages) {
            let ptr = frames.start_address() as *mut u8;
            self.pages_allocated.fetch_add(num_pages, Ordering::Relaxed);
            Some(ptr)
        } else {
            None
        }
    }

    unsafe fn try_alloc_from_free_list(&self, layout: Layout) -> Option<NonNull<u8>> {
        let size = layout.size().max(layout.align());
        let size = (size + 7) & !7;
        let mut free_lists = self.free_lists.lock();
        if let Some(ptr) = free_lists.remove_best_fit(size, layout.align()) {
            self.used_bytes.fetch_add(size, Ordering::Relaxed);
            Some(NonNull::new_unchecked(ptr))
        } else {
            None
        }
    }

    unsafe fn expand_heap(&self, layout: Layout) -> Option<NonNull<u8>> {
        let size = layout.size().max(layout.align());
        let size = (size + 4095) & !4095;
        if let Some(ptr) = self.alloc_pages_for_heap(size) {
            let mut free_lists = self.free_lists.lock();
            free_lists.insert(ptr, size);
            self.total_bytes.fetch_add(size, Ordering::Relaxed);
            drop(free_lists);
            self.try_alloc_from_free_list(layout)
        } else {
            None
        }
    }

    unsafe fn shrink_free_block(&self, ptr: *mut u8, requested_size: usize, block_size: usize) {
        if block_size > requested_size + 32 {
            let remaining = block_size - requested_size;
            if remaining >= 32 {
                let new_block_ptr = ptr.add(requested_size) as *mut u8;
                self.free_lists.lock().insert(new_block_ptr, remaining);
            }
        }
    }
}

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !self.initialized.load(Ordering::Relaxed) {
            return ptr::null_mut();
        }
        
        let size = layout.size().max(layout.align());
        let size = (size + 7) & !7;
        
        if let Some(ptr) = self.try_alloc_from_free_list(layout) {
            return ptr.as_ptr();
        }

        if let Some(ptr) = self.expand_heap(layout) {
            return ptr.as_ptr();
        }

        ptr::null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if !self.initialized.load(Ordering::Relaxed) {
            return;
        }
        
        let size = layout.size().max(layout.align());
        let size = (size + 7) & !7;
        self.used_bytes.fetch_sub(size, Ordering::Relaxed);
        self.free_lists.lock().insert(ptr, size);
    }
}

#[global_allocator]
static mut ALLOCATOR: Heap = Heap::new();

pub fn init_heap(start: usize, size: usize) {
    unsafe {
        ALLOCATOR.init(start, size);
    }
}

pub fn get_heap_usage() -> (usize, usize) {
    let used = unsafe { ALLOCATOR.used_bytes.load(Ordering::Relaxed) };
    let total = unsafe { ALLOCATOR.total_bytes.load(Ordering::Relaxed) };
    (used, total)
}

pub fn heap_stats() -> (usize, usize, usize) {
    let used = unsafe { ALLOCATOR.used_bytes.load(Ordering::Relaxed) };
    let total = unsafe { ALLOCATOR.total_bytes.load(Ordering::Relaxed) };
    let pages = unsafe { ALLOCATOR.pages_allocated.load(Ordering::Relaxed) };
    (used, total, pages)
}

unsafe fn validate_block(ptr: *mut u8) -> bool {
    let header_ptr = ptr.sub(core::mem::size_of::<BlockHeader>());
    let header = &*(header_ptr as *const BlockHeader);
    header.magic == BLOCK_MAGIC
}

pub fn alloc_zeroed(layout: Layout) -> *mut u8 {
    unsafe {
        if let Ok(ptr) = alloc::alloc::alloc_zeroed(layout) {
            ptr
        } else {
            ptr::null_mut()
        }
    }
}

pub fn realloc(old_ptr: *mut u8, old_layout: Layout, new_size: usize) -> *mut u8 {
    unsafe {
        let new_layout = Layout::from_size_align(new_size, old_layout.align()).unwrap();
        if new_layout.size() <= old_layout.size() {
            return old_ptr;
        }
        let new_ptr = alloc::alloc::alloc(new_layout);
        if !new_ptr.is_null() {
            core::ptr::copy_nonoverlapping(old_ptr, new_ptr, old_layout.size());
            alloc::alloc::dealloc(old_ptr, old_layout);
        }
        new_ptr
    }
}

pub fn aligned_alloc(align: usize, size: usize) -> *mut u8 {
    let layout = Layout::from_size_align(size, align).unwrap();
    unsafe { ALLOCATOR.alloc(layout) }
}

pub fn aligned_dealloc(ptr: *mut u8, align: usize, size: usize) {
    let layout = Layout::from_size_align(size, align).unwrap();
    unsafe { ALLOCATOR.dealloc(ptr, layout) }
}

pub fn collect_garbage() {
    // Stub implementation
}

pub fn defragment() {
    // Stub implementation
}

pub fn is_initialized() -> bool {
    unsafe { ALLOCATOR.initialized.load(Ordering::Relaxed) }
}

pub fn get_total_bytes() -> usize {
    unsafe { ALLOCATOR.total_bytes.load(Ordering::Relaxed) }
}

pub fn get_used_bytes() -> usize {
    unsafe { ALLOCATOR.used_bytes.load(Ordering::Relaxed) }
}

pub fn get_free_bytes() -> usize {
    let total = get_total_bytes();
    let used = get_used_bytes();
    if total > used { total - used } else { 0 }
}

pub fn get_pages_allocated() -> usize {
    unsafe { ALLOCATOR.pages_allocated.load(Ordering::Relaxed) }
}

pub fn validate_heap() -> bool {
    // Stub implementation
    true
}

pub fn heap_dump() {
    // Stub implementation
}

pub fn heap_verify_integrity() -> bool {
    // Stub implementation
    true
}

pub fn compact_heap() {
    // Stub implementation
}

pub fn trim_heap() {
    // Stub implementation
}

pub fn reserve_memory(pages: usize) -> bool {
    // Stub implementation
    false
}

pub fn release_reserved_memory(pages: usize) {
    // Stub implementation
}

pub fn set_heap_limit(limit: usize) {
    // Stub implementation
}

pub fn get_heap_limit() -> usize {
    usize::MAX
}

