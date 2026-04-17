use core::ptr;
use core::sync::atomic::{AtomicUsize, AtomicBool, AtomicPtr, Ordering};
use alloc::boxed::Box;
use spin::Mutex;
use lazy_static::lazy_static;
use crate::memory::virtual::{map_user_page, unmap_user_page};
use crate::scheduler::pcb::Process;

#[repr(C)]
pub struct SyscallArgs {
    pub nr: u64,
    pub a1: u64,
    pub a2: u64,
    pub a3: u64,
    pub a4: u64,
    pub a5: u64,
    pub a6: u64,
}

const SYS_EXIT: u64 = 60;
const SYS_READ: u64 = 0;
const SYS_WRITE: u64 = 1;
const SYS_MMAP: u64 = 9;
const SYS_BRK: u64 = 12;
const SYS_EXECVE: u64 = 59;
const SYS_FORK: u64 = 57;

// Flags for mmap
const PROT_READ: u64 = 1;
const PROT_WRITE: u64 = 2;
const PROT_EXEC: u64 = 4;
const MAP_SHARED: u64 = 1;
const MAP_PRIVATE: u64 = 2;
const MAP_ANONYMOUS: u64 = 0x20;
const MAP_FIXED: u64 = 0x10;

// Virtual memory bounds (ASLR-friendly)
const USER_SPACE_START: usize = 0x40000000;
const USER_SPACE_END: usize = 0x7FFFFFFFFF000; // Below kernel space

// Physical memory bounds
const PHYSICAL_START: usize = 0x1000000; // Start at 16MB
const PHYSICAL_END: usize = 0x80000000; // End at 2GB (adjustable)

// Page size
const PAGE_SIZE: usize = 4096;

// Per-process heap management
#[derive(Debug)]
pub struct PageAllocator {
    start_addr: AtomicUsize,
    end_addr: AtomicUsize,
    current: AtomicUsize,
    lock: AtomicBool,
}

impl PageAllocator {
    pub fn new(heap_start: usize, heap_size: usize) -> Self {
        let heap_end = heap_start.saturating_add(heap_size);
        PageAllocator {
            start_addr: AtomicUsize::new(heap_start),
            end_addr: AtomicUsize::new(heap_end),
            current: AtomicUsize::new(heap_start),
            lock: AtomicBool::new(false),
        }
    }

    pub fn allocate_pages(&self, count: usize) -> Option<usize> {
        if count == 0 {
            return None;
        }
        
        let total_size = count * PAGE_SIZE;
        
        loop {
            // Acquire lock
            while self.lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
                core::hint::spin_loop();
            }
            
            let current = self.current.load(Ordering::SeqCst);
            let new_current = current.saturating_add(total_size);
            let end_addr = self.end_addr.load(Ordering::SeqCst);
            
            if new_current > end_addr {
                self.lock.store(false, Ordering::Release);
                return None;
            }
            
            match self.current.compare_exchange(
                current, 
                new_current, 
                Ordering::SeqCst, 
                Ordering::SeqCst
            ) {
                Ok(_) => {
                    self.lock.store(false, Ordering::Release);
                    return Some(current);
                }
                Err(_) => {
                    // Another thread updated current, retry
                    self.lock.store(false, Ordering::Release);
                }
            }
        }
    }
    
    pub fn deallocate_pages(&self, _addr: usize, _count: usize) -> Result<(), &'static str> {
        // Bump allocator doesn't support deallocation
        // In a real system, this would require a more sophisticated allocator
        Err("Bump allocator does not support deallocation")
    }
    
    pub fn get_current(&self) -> usize {
        self.current.load(Ordering::SeqCst)
    }
    
    pub fn set_current(&self, new_current: usize) -> Result<(), &'static str> {
        let start_addr = self.start_addr.load(Ordering::SeqCst);
        let end_addr = self.end_addr.load(Ordering::SeqCst);
        
        if new_current < start_addr || new_current > end_addr {
            return Err("Address out of bounds");
        }
        
        self.current.store(new_current, Ordering::SeqCst);
        Ok(())
    }
}

// Physical page allocator with proper bitmap management
pub struct PhysicalPageAllocator {
    start_addr: usize,
    end_addr: usize,
    bitmap: Box<[AtomicUsize]>,
    total_pages: usize,
    next_free_hint: AtomicUsize, // Hint for next free page search
    lock: AtomicBool,
}

impl PhysicalPageAllocator {
    pub fn new(start: usize, end: usize) -> Result<Self, &'static str> {
        if start % PAGE_SIZE != 0 || end % PAGE_SIZE != 0 {
            return Err("Start and end must be page-aligned");
        }
        
        let total_pages = (end - start) / PAGE_SIZE;
        let bitmap_entries = (total_pages + 63) / 64; // Number of AtomicUsize entries needed
        
        // Create bitmap
        let mut bitmap_vec = Vec::with_capacity(bitmap_entries);
        for _ in 0..bitmap_entries {
            bitmap_vec.push(AtomicUsize::new(0));
        }
        
        Ok(PhysicalPageAllocator {
            start_addr: start,
            end_addr: end,
            bitmap: bitmap_vec.into_boxed_slice(),
            total_pages,
            next_free_hint: AtomicUsize::new(0),
            lock: AtomicBool::new(false),
        })
    }

    pub fn allocate_page(&self) -> Option<usize> {
        loop {
            // Acquire lock
            while self.lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
                core::hint::spin_loop();
            }
            
            let mut page_idx = self.next_free_hint.load(Ordering::SeqCst);
            
            // Search for free page starting from hint
            for _ in 0..self.total_pages {
                if page_idx >= self.total_pages {
                    page_idx = 0; // Wrap around
                }
                
                let entry_idx = page_idx / 64;
                let bit_idx = page_idx % 64;
                
                if entry_idx >= self.bitmap.len() {
                    break;
                }
                
                let old_entry = self.bitmap[entry_idx].load(Ordering::SeqCst);
                let mask = 1u64 << bit_idx;
                
                if (old_entry & mask as usize) == 0 {
                    // Try to atomically set the bit
                    match self.bitmap[entry_idx].compare_exchange_weak(
                        old_entry,
                        old_entry | (mask as usize),
                        Ordering::SeqCst,
                        Ordering::SeqCst
                    ) {
                        Ok(_) => {
                            // Update hint for next allocation
                            self.next_free_hint.store((page_idx + 1) % self.total_pages, Ordering::SeqCst);
                            
                            self.lock.store(false, Ordering::Release);
                            return Some(self.start_addr + (page_idx * PAGE_SIZE));
                        }
                        Err(_) => {
                            // Another thread took this page, continue searching
                            continue;
                        }
                    }
                }
                
                page_idx += 1;
            }
            
            // Release lock and return None if no free pages found
            self.lock.store(false, Ordering::Release);
            return None;
        }
    }
    
    pub fn deallocate_page(&self, addr: usize) -> Result<(), &'static str> {
        if addr < self.start_addr || addr >= self.end_addr || (addr - self.start_addr) % PAGE_SIZE != 0 {
            return Err("Invalid physical address for deallocation");
        }
        
        // Acquire lock
        while self.lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            core::hint::spin_loop();
        }
        
        let page_idx = (addr - self.start_addr) / PAGE_SIZE;
        let entry_idx = page_idx / 64;
        let bit_idx = page_idx % 64;
        
        if entry_idx >= self.bitmap.len() {
            self.lock.store(false, Ordering::Release);
            return Err("Page index out of bounds");
        }
        
        let mask = 1u64 << bit_idx;
        let old_entry = self.bitmap[entry_idx].fetch_and(!(mask as usize), Ordering::SeqCst);
        
        if (old_entry & (mask as usize)) == 0 {
            // Page was already free
            self.lock.store(false, Ordering::Release);
            return Err("Page was already deallocated");
        }
        
        self.lock.store(false, Ordering::Release);
        Ok(())
    }
    
    pub fn is_allocated(&self, addr: usize) -> Result<bool, &'static str> {
        if addr < self.start_addr || addr >= self.end_addr || (addr - self.start_addr) % PAGE_SIZE != 0 {
            return Err("Invalid physical address");
        }
        
        let page_idx = (addr - self.start_addr) / PAGE_SIZE;
        let entry_idx = page_idx / 64;
        let bit_idx = page_idx % 64;
        
        if entry_idx >= self.bitmap.len() {
            return Err("Page index out of bounds");
        }
        
        let entry = self.bitmap[entry_idx].load(Ordering::SeqCst);
        Ok((entry & (1u64 << bit_idx) as usize) != 0)
    }
}

// Global physical allocator
lazy_static! {
    pub static ref PHYSICAL_ALLOCATOR: Mutex<PhysicalPageAllocator> = 
        Mutex::new(PhysicalPageAllocator::new(PHYSICAL_START, PHYSICAL_END).unwrap());
}

pub fn handle_core_syscall(args: &SyscallArgs) -> u64 {
    // Validate syscall number
    if args.nr > 1000 { // Reasonable upper limit
        return -1i64 as u64;
    }

    // Validate pointers based on syscall type
    match args.nr {
        SYS_READ | SYS_WRITE => {
            if !validate_user_pointer(args.a2 as *const (), args.a3 as usize) {
                return -1i64 as u64;
            }
        }
        SYS_MMAP => {
            // Validate length and flags
            if args.a2 == 0 || args.a2 > 0x1000000000 { // Max 64GB
                return -1i64 as u64;
            }
        }
        SYS_BRK => {
            if args.a1 != 0 && !validate_user_pointer(args.a1 as *const (), 0) {
                return -1i64 as u64;
            }
        }
        _ => {} // Other syscalls don't need pointer validation here
    }

    match args.nr {
        SYS_EXIT => sys_exit(args.a1 as i32),
        SYS_READ => sys_read(args.a1 as i32, args.a2 as *mut u8, args.a3),
        SYS_WRITE => sys_write(args.a1 as i32, args.a2 as *const u8, args.a3),
        SYS_MMAP => sys_mmap(args.a1 as *const (), args.a2, args.a3, args.a4, args.a5 as i32, args.a6),
        SYS_BRK => sys_brk(args.a1 as *mut ()),
        SYS_EXECVE => sys_execve(args.a1 as *const u8, args.a2 as *const *const u8, args.a3 as *const *const u8),
        SYS_FORK => sys_fork(),
        _ => -1i64 as u64,
    }
}

fn validate_user_pointer(ptr: *const (), len: usize) -> bool {
    // Check if pointer is null
    if ptr.is_null() {
        return false;
    }
    
    let addr = ptr as usize;
    
    // Check if address is in user space range
    if addr < USER_SPACE_START || addr >= USER_SPACE_END {
        return false;
    }
    
    // Check for overflow when adding length
    if len > 0 && addr.checked_add(len).is_none() {
        return false;
    }
    
    // Check if the end of the range is still in user space
    if len > 0 && (addr + len) > USER_SPACE_END {
        return false;
    }
    
    true
}

fn validate_c_string(ptr: *const u8) -> Result<usize, ()> {
    if ptr.is_null() {
        return Err(());
    }
    
    let mut len = 0;
    let mut current_ptr = ptr;
    
    // Limit string length to prevent infinite loops
    while len < 4096 {
        let byte = unsafe { ptr::read_volatile(current_ptr) };
        if byte == 0 {
            return Ok(len);
        }
        current_ptr = unsafe { current_ptr.add(1) };
        len += 1;
    }
    
    Err(()) // String too long
}

fn sys_exit(status: i32) -> u64 {
    unsafe { 
        let current = Process::current();
        if let Some(process) = current {
            // Deallocate memory resources used by the process
            process.deallocate_memory_resources();
            process.exit(status); 
        }
    }
    loop {}
}

fn console_get_byte() -> Option<u8> {
    // Use configurable addresses instead of hardcoded ones
    let uart_base = if cfg!(target_arch = "x86_64") {
        0x3F8
    } else if cfg!(target_arch = "aarch64") {
        0x9000000 // Example PL011 base address
    } else if cfg!(target_arch = "riscv64") {
        0x10000000
    } else {
        return None; // Unsupported architecture
    };

    unsafe {
        let status = ptr::read_volatile(uart_base as *const u8);
        if (status & 0x01) != 0 {
            return Some(ptr::read_volatile((uart_base + 5) as *const u8)); // Data register is usually offset 5
        }
    }
    None
}

fn sys_read(fd: i32, buf: *mut u8, count: u64) -> u64 {
    if buf.is_null() || count == 0 {
        return -1i64 as u64;
    }
    
    // Validate that buffer is accessible by user process
    if !validate_user_pointer(buf as *const (), count as usize) {
        return -1i64 as u64;
    }
    
    let count = count.min(i32::MAX as u64) as usize;
    let buffer = unsafe { 
        core::slice::from_raw_parts_mut(buf, count) 
    };
    
    match fd {
        0 => {
            // Read from stdin
            let mut bytes_read = 0;
            for b in buffer.iter_mut().take(count) {
                if let Some(byte) = console_get_byte() {
                    *b = byte;
                    bytes_read += 1;
                } else {
                    // No more input available immediately
                    break;
                }
            }
            bytes_read as u64
        }
        _ => {
            // In a real system, this would access the VFS
            -1i64 as u64
        }
    }
}

fn console_put_byte(byte: u8) {
    let uart_base = if cfg!(target_arch = "x86_64") {
        0x3F8
    } else if cfg!(target_arch = "aarch64") {
        0x9000000
    } else if cfg!(target_arch = "riscv64") {
        0x10000000
    } else {
        return; // Unsupported architecture
    };

    // Wait until transmit holding register is empty (timeout-based approach would be better)
    unsafe {
        // Wait for THRE bit to be set
        while (ptr::read_volatile(uart_base as *const u8) & 0x20) == 0 {
            core::hint::spin_loop();
        }
        ptr::write_volatile((uart_base + 0) as *mut u8, byte); // Write to data register
    }
}

fn sys_write(fd: i32, buf: *const u8, count: u64) -> u64 {
    if buf.is_null() || count == 0 {
        return -1i64 as u64;
    }
    
    // Validate that buffer is accessible by user process
    if !validate_user_pointer(buf as *const (), count as usize) {
        return -1i64 as u64;
    }
    
    let count = count.min(i32::MAX as u64) as usize;
    let slice = unsafe { 
        core::slice::from_raw_parts(buf, count) 
    };
    
    match fd {
        1 | 2 => {
            // Write to stdout/stderr using console output
            for &byte in slice.iter().take(count) {
                console_put_byte(byte);
            }
            count as u64
        },
        _ => {
            // In a real system, this would access the VFS
            -1i64 as u64
        }
    }
}

fn sys_mmap(addr: *const (), length: u64, prot: u64, flags: u64, fd: i32, offset: u64) -> u64 {
    if length == 0 {
        return -1i64 as u64;
    }
    
    // Validate length
    if length > 0x1000000000 { // 64GB max
        return -1i64 as u64;
    }
    
    // Check protection flags
    let valid_prot = prot & !(PROT_READ | PROT_WRITE | PROT_EXEC) == 0;
    if !valid_prot {
        return -1i64 as u64;
    }
    
    // Check mapping flags
    let valid_flags = flags & !(MAP_SHARED | MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED) == 0;
    if !valid_flags {
        return -1i64 as u64;
    }
    
    let pages_needed = ((length as usize + PAGE_SIZE - 1) / PAGE_SIZE).max(1);
    
    // Determine virtual address
    let virt_addr = if (flags & MAP_FIXED) != 0 && !addr.is_null() {
        let addr_val = addr as usize;
        if !validate_user_pointer(addr as *const (), length as usize) {
            return -1i64 as u64;
        }
        addr_val
    } else {
        // Allocate virtual address space from process-specific allocator
        let current_process = unsafe { Process::current() };
        if current_process.is_none() {
            return -1i64 as u64;
        }
        
        let allocator = current_process.unwrap().get_page_allocator();
        match allocator.allocate_pages(pages_needed) {
            Some(addr) => addr,
            None => return -1i64 as u64,
        }
    };
    
    // Check for existing mappings that would overlap
    if (flags & MAP_FIXED) == 0 {
        // In a real system, we'd check the page table for overlaps
        // For now, assume no overlap since we're using a simple allocator
    }
    
    // Map each page
    for i in 0..pages_needed {
        let vaddr = virt_addr + (i * PAGE_SIZE);
        
        let phys_addr = if (flags & MAP_ANONYMOUS) != 0 {
            // Anonymous mapping - allocate physical page
            match PHYSICAL_ALLOCATOR.lock().allocate_page() {
                Some(addr) => addr,
                None => {
                    // Clean up previously allocated pages on failure
                    for j in 0..i {
                        let vaddr_to_unmap = virt_addr + (j * PAGE_SIZE);
                        unmap_user_page(vaddr_to_unmap);
                        // Deallocate corresponding physical page
                        let prev_vaddr = virt_addr + (j * PAGE_SIZE);
                        // We'd need to track which physical page was mapped to prev_vaddr
                        // This requires a reverse mapping which we don't have yet
                    }
                    return -1i64 as u64;
                }
            }
        } else {
            // File-backed mapping - for now, treat as anonymous
            match PHYSICAL_ALLOCATOR.lock().allocate_page() {
                Some(addr) => addr,
                None => {
                    for j in 0..i {
                        let vaddr_to_unmap = virt_addr + (j * PAGE_SIZE);
                        unmap_user_page(vaddr_to_unmap);
                    }
                    return -1i64 as u64;
                }
            }
        };
        
        let readable = (prot & PROT_READ) != 0;
        let writable = (prot & PROT_WRITE) != 0;
        let executable = (prot & PROT_EXEC) != 0;
        
        if !map_user_page(vaddr, phys_addr, readable, writable, executable) {
            // Clean up previously mapped pages
            for j in 0..i {
                let vaddr_to_unmap = virt_addr + (j * PAGE_SIZE);
                unmap_user_page(vaddr_to_unmap);
            }
            // Deallocate the physical page that failed to map
            if let Err(_) = PHYSICAL_ALLOCATOR.lock().deallocate_page(phys_addr) {
                // Log error but continue cleanup
            }
            return -1i64 as u64;
        }
    }
    
    virt_addr as u64
}

fn sys_brk(new_brk: *mut ()) -> u64 {
    let current_process = unsafe { Process::current() };
    if current_process.is_none() {
        return -1i64 as u64;
    }
    
    let process = current_process.unwrap();
    let heap_start = process.get_heap_start();
    let heap_end = process.get_heap_end();
    let max_heap = process.get_max_heap_address();
    
    if new_brk.is_null() {
        // Return current break value
        return heap_end as u64;
    }
    
    let new_brk_addr = new_brk as usize;
    
    // Validate address range
    if new_brk_addr < heap_start || new_brk_addr > max_heap {
        return -1i64 as u64;
    }
    
    // Check if we're expanding or contracting the heap
    if new_brk_addr > heap_end {
        // Expanding - allocate pages
        let pages_needed = (new_brk_addr - heap_end + PAGE_SIZE - 1) / PAGE_SIZE;
        
        for i in 0..pages_needed {
            let vaddr = heap_end + (i * PAGE_SIZE);
            if let Some(phys_addr) = PHYSICAL_ALLOCATOR.lock().allocate_page() {
                if !map_user_page(vaddr, phys_addr, true, true, false) {
                    // Failed to map, clean up
                    for j in 0..i {
                        let vaddr_to_unmap = heap_end + (j * PAGE_SIZE);
                        unmap_user_page(vaddr_to_unmap);
                        // Deallocate physical page
                        let temp_phys_addr = phys_addr - (i * PAGE_SIZE) + (j * PAGE_SIZE);
                        let _ = PHYSICAL_ALLOCATOR.lock().deallocate_page(temp_phys_addr);
                    }
                    return -1i64 as u64;
                }
            } else {
                // Failed to allocate physical page, clean up
                for j in 0..i {
                    let vaddr_to_unmap = heap_end + (j * PAGE_SIZE);
                    unmap_user_page(vaddr_to_unmap);
                }
                return -1i64 as u64;
            }
        }
    } else if new_brk_addr < heap_end {
        // Contracting - unmap pages
        let pages_to_unmap = (heap_end - new_brk_addr + PAGE_SIZE - 1) / PAGE_SIZE;
        
        for i in 0..pages_to_unmap {
            let vaddr = new_brk_addr + (i * PAGE_SIZE);
            if vaddr < heap_end {
                unmap_user_page(vaddr);
                // We can't easily determine the physical address to deallocate
                // without a reverse mapping
            }
        }
    }
    
    // Update heap end
    process.set_heap_end(new_brk_addr);
    new_brk_addr as u64
}

fn sys_execve(pathname: *const u8, argv: *const *const u8, envp: *const *const u8) -> u64 {
    if pathname.is_null() {
        return -1i64 as u64;
    }
    
    // Validate path string
    let path_len = match validate_c_string(pathname) {
        Ok(len) => len,
        Err(_) => return -1i64 as u64,
    };
    
    if path_len == 0 {
        return -1i64 as u64;
    }
    
    // In a real implementation, this would:
    // 1. Validate all argv and envp strings
    // 2. Locate and read the executable file
    // 3. Parse ELF header
    // 4. Load program segments into memory
    // 5. Set up new process memory layout
    // 6. Replace current process image
    
    // For now, return error as placeholder
    -1i64 as u64
}

fn sys_fork() -> u64 {
    let current = unsafe { Process::current() };
    if current.is_none() {
        return -1i64 as u64;
    }
    
    match current.unwrap().fork() {
        Ok(child) => {
            // In the parent process, return the child's PID
            child.get_pid() as u64
        }
        Err(_) => {
            // Fork failed
            -1i64 as u64
        }
    }
}

pub fn allocate_physical_page() -> Option<usize> {
    PHYSICAL_ALLOCATOR.lock().allocate_page()
}

pub fn deallocate_physical_page(addr: usize) -> Result<(), &'static str> {
    PHYSICAL_ALLOCATOR.lock().deallocate_page(addr)
}

