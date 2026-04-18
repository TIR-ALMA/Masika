pub mod physical;
pub mod virtual_memory;
pub mod heap;

pub use physical::*;
pub use virtual_memory::*;
pub use heap::*;

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_MASK: usize = 0xFFFFFFFFFFFFF000;

pub type PhysAddr = usize;
pub type VirtAddr = usize;

#[derive(Clone, Copy)]
pub struct Page {
    pub addr: PhysAddr,
}

impl Page {
    pub fn new(addr: PhysAddr) -> Self {
        Self { addr }
    }

    pub fn as_ptr<T>(&self) -> *const T {
        self.addr as *const T
    }

    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.addr as *mut T
    }
}

pub trait FrameAllocator {
    fn alloc_frame(&mut self) -> Option<Page>;
    fn dealloc_frame(&mut self, page: Page);
}

pub fn init() {
    // Инициализация памяти
    physical::init();
    virtual_memory::init();
    heap::init();
}

