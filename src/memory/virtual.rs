use core::ptr;
use core::sync::atomic::{AtomicUsize, AtomicBool, AtomicPtr, Ordering};
use core::cmp::min;
use crate::arch::paging::{PML4, PageTable, PageDirectory, PageDirectoryPointer, PageEntry};
use crate::memory::physical::{alloc_pages, free_pages, PhysAddr, PageFrame};

static USER_SPACE_BASE: u64 = 0x100000;
static USER_SPACE_END: u64 = 0x0000800000000000;
static KERNEL_SPACE_START: u64 = 0xFFFF800000000000;

pub struct VirtualMemoryManager {
    pml4: *mut PML4,
    pid: usize,
    page_count: AtomicUsize,
    cow_enabled: AtomicBool,
    heap_start: VirtAddr,
    heap_end: VirtAddr,
    brk: AtomicPtr<u8>,
}

#[derive(Debug)]
pub enum VmmError {
    OutOfMemory,
    InvalidAddress,
    AccessDenied,
    RangeOverlap,
}

pub fn init() {
    // Initialization logic here
}

impl VirtualMemoryManager {
    pub fn new() -> Self {
        let pml4_frame = alloc_pages(1).unwrap();
        let pml4 = pml4_frame.as_mut_ptr::<PML4>();
        unsafe { ptr::write_bytes(pml4, 0, 1); }
        
        let heap_start = VirtAddr(USER_SPACE_BASE + 0x1000000);
        let heap_end = VirtAddr(heap_start.0 + 0x10000000);
        
        Self {
            pml4,
            pid: 0,
            page_count: AtomicUsize::new(0),
            cow_enabled: AtomicBool::new(true),
            heap_start,
            heap_end,
            brk: AtomicPtr::new(heap_start.0 as *mut u8),
        }
    }

    pub fn with_pid(pid: usize) -> Self {
        let pml4_frame = alloc_pages(1).unwrap();
        let pml4 = pml4_frame.as_mut_ptr::<PML4>();
        unsafe { ptr::write_bytes(pml4, 0, 1); }
        
        let heap_start = VirtAddr(USER_SPACE_BASE + 0x1000000);
        let heap_end = VirtAddr(heap_start.0 + 0x10000000);
        
        Self {
            pml4,
            pid,
            page_count: AtomicUsize::new(0),
            cow_enabled: AtomicBool::new(true),
            heap_start,
            heap_end,
            brk: AtomicPtr::new(heap_start.0 as *mut u8),
        }
    }

    pub fn switch_to(&self) {
        unsafe {
            asm!("mov cr3, {}", in(reg) self.pml4 as u64);
        }
    }

    pub fn get_current_pml4(&self) -> u64 {
        self.pml4 as u64
    }

    pub fn map_page(&mut self, virt: VirtAddr, phys: PhysAddr, flags: u64) -> Result<(), VmmError> {
        let pml4_index = (virt.0 >> 39) & 0x1FF;
        let pdpt_index = (virt.0 >> 30) & 0x1FF;
        let pd_index = (virt.0 >> 21) & 0x1FF;
        let pt_index = (virt.0 >> 12) & 0x1FF;

        let pdpt_entry = unsafe { &mut (*self.pml4).entries[pml4_index] };
        if pdpt_entry.is_unused() {
            let frame = alloc_pages(1).ok_or(VmmError::OutOfMemory)?;
            pdpt_entry.set(frame, 0x03);
            self.page_count.fetch_add(1, Ordering::SeqCst);
        }
        let pdpt = PhysAddr(pdpt_entry.addr()).as_mut_ptr::<PageDirectoryPointer>();

        let pd_entry = unsafe { &mut (*pdpt).entries[pdpt_index] };
        if pd_entry.is_unused() {
            let frame = alloc_pages(1).ok_or(VmmError::OutOfMemory)?;
            pd_entry.set(frame, 0x03);
            self.page_count.fetch_add(1, Ordering::SeqCst);
        }
        let pd = PhysAddr(pd_entry.addr()).as_mut_ptr::<PageDirectory>();

        let pt_entry = unsafe { &mut (*pd).entries[pd_index] };
        if pt_entry.is_unused() {
            let frame = alloc_pages(1).ok_or(VmmError::OutOfMemory)?;
            pt_entry.set(frame, 0x03);
            self.page_count.fetch_add(1, Ordering::SeqCst);
        }
        let pt = PhysAddr(pt_entry.addr()).as_mut_ptr::<PageTable>();

        let pte = unsafe { &mut (*pt).entries[pt_index] };
        pte.set(phys, flags);
        self.page_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub fn unmap_page(&mut self, virt: VirtAddr) -> Result<(), VmmError> {
        let pml4_index = (virt.0 >> 39) & 0x1FF;
        let pdpt_index = (virt.0 >> 30) & 0x1FF;
        let pd_index = (virt.0 >> 21) & 0x1FF;
        let pt_index = (virt.0 >> 12) & 0x1FF;

        let pdpt_entry = unsafe { &(*self.pml4).entries[pml4_index] };
        if pdpt_entry.is_unused() { return Err(VmmError::InvalidAddress); }
        let pdpt = PhysAddr(pdpt_entry.addr()).as_ptr::<PageDirectoryPointer>();

        let pd_entry = unsafe { &(*pdpt).entries[pdpt_index] };
        if pd_entry.is_unused() { return Err(VmmError::InvalidAddress); }
        let pd = PhysAddr(pd_entry.addr()).as_ptr::<PageDirectory>();

        let pt_entry = unsafe { &(*pd).entries[pd_index] };
        if pt_entry.is_unused() { return Err(VmmError::InvalidAddress); }
        let pt = PhysAddr(pt_entry.addr()).as_ptr::<PageTable>();

        let pte = unsafe { &(*pt).entries[pt_index] };
        if !pte.is_unused() {
            unsafe { (*pt).entries[pt_index].set_unused(); }
            self.page_count.fetch_sub(1, Ordering::SeqCst);
        }
        Ok(())
    }

    pub fn map_range(&mut self, start: VirtAddr, size: usize, phys_start: PhysAddr, flags: u64) -> Result<(), VmmError> {
        let pages = (size + 0xFFF) / 0x1000;
        for i in 0..pages {
            let vaddr = VirtAddr(start.0 + (i as u64) * 0x1000);
            let paddr = PhysAddr(phys_start.0 + (i as u64) * 0x1000);
            self.map_page(vaddr, paddr, flags)?;
        }
        Ok(())
    }

    pub fn unmap_range(&mut self, start: VirtAddr, size: usize) -> Result<(), VmmError> {
        let pages = (size + 0xFFF) / 0x1000;
        for i in 0..pages {
            let vaddr = VirtAddr(start.0 + (i as u64) * 0x1000);
            self.unmap_page(vaddr)?;
        }
        Ok(())
    }

    pub fn handle_page_fault(&mut self, addr: u64, error_code: u64) -> Result<(), VmmError> {
        let vaddr = VirtAddr(addr & !0xFFF);
        if error_code & 0x01 == 0 {
            let frame = alloc_pages(1).ok_or(VmmError::OutOfMemory)?;
            self.map_page(vaddr, frame, 0x03)?;
        } else if error_code & 0x02 != 0 && self.is_cow_page(vaddr) {
            self.handle_cow_fault(vaddr)?;
        } else {
            return Err(VmmError::AccessDenied);
        }
        Ok(())
    }

    fn is_cow_page(&self, addr: VirtAddr) -> bool {
        if !self.cow_enabled.load(Ordering::Relaxed) {
            return false;
        }
        let pml4_index = (addr.0 >> 39) & 0x1FF;
        let pdpt_index = (addr.0 >> 30) & 0x1FF;
        let pd_index = (addr.0 >> 21) & 0x1FF;
        let pt_index = (addr.0 >> 12) & 0x1FF;

        let pdpt_entry = unsafe { &(*self.pml4).entries[pml4_index] };
        if pdpt_entry.is_unused() { return false; }
        let pdpt = PhysAddr(pdpt_entry.addr()).as_ptr::<PageDirectoryPointer>();

        let pd_entry = unsafe { &(*pdpt).entries[pdpt_index] };
        if pd_entry.is_unused() { return false; }
        let pd = PhysAddr(pd_entry.addr()).as_ptr::<PageDirectory>();

        let pt_entry = unsafe { &(*pd).entries[pd_index] };
        if pt_entry.is_unused() { return false; }
        let pt = PhysAddr(pt_entry.addr()).as_ptr::<PageTable>();

        let pte = unsafe { &(*pt).entries[pt_index] };
        pte.flags() & 0x20 != 0
    }

    fn handle_cow_fault(&mut self, addr: VirtAddr) -> Result<(), VmmError> {
        let frame = alloc_pages(1).ok_or(VmmError::OutOfMemory)?;
        let old_phys = self.translate(addr).ok_or(VmmError::InvalidAddress)?;
        self.map_page(addr, frame, 0x07)?;
        
        unsafe {
            ptr::copy_nonoverlapping(
                old_phys.as_ptr::<u8>(),
                frame.as_mut_ptr::<u8>(),
                0x1000
            );
        }
        Ok(())
    }

    pub fn translate(&self, virt: VirtAddr) -> Option<PhysAddr> {
        let pml4_index = (virt.0 >> 39) & 0x1FF;
        let pdpt_index = (virt.0 >> 30) & 0x1FF;
        let pd_index = (virt.0 >> 21) & 0x1FF;
        let pt_index = (virt.0 >> 12) & 0x1FF;

        let pdpt_entry = unsafe { &(*self.pml4).entries[pml4_index] };
        if pdpt_entry.is_unused() { return None; }
        let pdpt = PhysAddr(pdpt_entry.addr()).as_ptr::<PageDirectoryPointer>();

        let pd_entry = unsafe { &(*pdpt).entries[pdpt_index] };
        if pd_entry.is_unused() { return None; }
        let pd = PhysAddr(pd_entry.addr()).as_ptr::<PageDirectory>();

        let pt_entry = unsafe { &(*pd).entries[pd_index] };
        if pt_entry.is_unused() { return None; }
        let pt = PhysAddr(pt_entry.addr()).as_ptr::<PageTable>();

        let pte = unsafe { &(*pt).entries[pt_index] };
        if pte.is_unused() { return None; }
        Some(PhysAddr(pte.addr()))
    }

    pub fn copy_to_user(&self, dest: *mut u8, src: &[u8]) -> Result<(), VmmError> {
        if !self.validate_user_range(VirtAddr(dest as u64), src.len()) {
            return Err(VmmError::AccessDenied);
        }
        unsafe {
            ptr::copy_nonoverlapping(src.as_ptr(), dest, src.len());
        }
        Ok(())
    }

    pub fn copy_from_user(&self, dest: &mut [u8], src: *const u8) -> Result<(), VmmError> {
        if !self.validate_user_range(VirtAddr(src as u64), dest.len()) {
            return Err(VmmError::AccessDenied);
        }
        unsafe {
            ptr::copy_nonoverlapping(src, dest.as_mut_ptr(), dest.len());
        }
        Ok(())
    }

    pub fn copy_string_from_user(&self, buf: &mut [u8], src: *const u8) -> Result<usize, VmmError> {
        if !self.validate_user_range(VirtAddr(src as u64), 1) {
            return Err(VmmError::AccessDenied);
        }
        let mut copied = 0;
        unsafe {
            while copied < buf.len() - 1 {
                if !self.validate_user_range(VirtAddr((src as u64) + copied as u64), 1) {
                    return Err(VmmError::AccessDenied);
                }
                let byte = ptr::read_volatile((src as *const u8).add(copied));
                buf[copied] = byte;
                if byte == 0 {
                    break;
                }
                copied += 1;
            }
            buf[copied] = 0;
        }
        Ok(copied)
    }

    fn validate_user_range(&self, start: VirtAddr, len: usize) -> bool {
        let end = start.0 + len as u64;
        if end >= USER_SPACE_END || start.0 >= USER_SPACE_END {
            return false;
        }
        if start.0 < USER_SPACE_BASE {
            return false;
        }
        true
    }

    pub fn allocate_user_pages(&mut self, count: usize, flags: u64) -> Result<VirtAddr, VmmError> {
        let frames = alloc_pages(count).ok_or(VmmError::OutOfMemory)?;
        let start_vaddr = self.find_free_virtual_range(count)?;
        
        for i in 0..count {
            let vaddr = VirtAddr(start_vaddr.0 + (i as u64) * 0x1000);
            let paddr = PhysAddr(frames.0 + (i as u64) * 0x1000);
            self.map_page(vaddr, paddr, flags)?;
        }
        Ok(start_vaddr)
    }

    fn find_free_virtual_range(&self, count: usize) -> Result<VirtAddr, VmmError> {
        let mut base = USER_SPACE_BASE;
        loop {
            if base >= USER_SPACE_END {
                return Err(VmmError::OutOfMemory);
            }
            let mut found = true;
            for i in 0..count {
                let vaddr = VirtAddr(base + (i as u64) * 0x1000);
                if self.translate(vaddr).is_some() {
                    found = false;
                    base = (base + 0x1000) & !0xFFF;
                    break;
                }
            }
            if found {
                return Ok(VirtAddr(base));
            }
        }
    }

    pub fn mark_page_cow(&mut self, addr: VirtAddr) {
        let pml4_index = (addr.0 >> 39) & 0x1FF;
        let pdpt_index = (addr.0 >> 30) & 0x1FF;
        let pd_index = (addr.0 >> 21) & 0x1FF;
        let pt_index = (addr.0 >> 12) & 0x1FF;

        let pdpt_entry = unsafe { &mut (*self.pml4).entries[pml4_index] };
        if pdpt_entry.is_unused() { return; }
        let pdpt = PhysAddr(pdpt_entry.addr()).as_mut_ptr::<PageDirectoryPointer>();

        let pd_entry = unsafe { &mut (*pdpt).entries[pdpt_index] };
        if pd_entry.is_unused() { return; }
        let pd = PhysAddr(pd_entry.addr()).as_mut_ptr::<PageDirectory>();

        let pt_entry = unsafe { &mut (*pd).entries[pd_index] };
        if pt_entry.is_unused() { return; }
        let pt = PhysAddr(pt_entry.addr()).as_mut_ptr::<PageTable>();

        let pte = unsafe { &mut (*pt).entries[pt_index] };
        if !pte.is_unused() {
            pte.update_flags(pte.flags() & !0x01 | 0x20);
        }
    }

    pub fn enable_cow(&self) {
        self.cow_enabled.store(true, Ordering::Relaxed);
    }

    pub fn disable_cow(&self) {
        self.cow_enabled.store(false, Ordering::Relaxed);
    }

    pub fn get_used_pages(&self) -> usize {
        self.page_count.load(Ordering::SeqCst)
    }

    pub fn clone_address_space(&self) -> Result<VirtualMemoryManager, VmmError> {
        let new_pml4_frame = alloc_pages(1).ok_or(VmmError::OutOfMemory)?;
        let new_pml4 = new_pml4_frame.as_mut_ptr::<PML4>();
        unsafe { ptr::write_bytes(new_pml4, 0, 1); }

        let mut new_manager = VirtualMemoryManager {
            pml4: new_pml4,
            pid: self.pid,
            page_count: AtomicUsize::new(0),
            cow_enabled: AtomicBool::new(self.cow_enabled.load(Ordering::Relaxed)),
            heap_start: self.heap_start,
            heap_end: self.heap_end,
            brk: AtomicPtr::new(self.brk.load(Ordering::SeqCst)),
        };
        self.clone_tables_recursive(0, new_manager.pml4)?;
        Ok(new_manager)
    }

    fn clone_tables_recursive(&self, level: u8, new_pml4: *mut PML4) -> Result<(), VmmError> {
        match level {
            0 => {
                for i in 0..512 {
                    let entry = unsafe { &(*self.pml4).entries[i] };
                    if !entry.is_unused() && entry.addr() != 0 {
                        let new_pdpt_frame = alloc_pages(1).ok_or(VmmError::OutOfMemory)?;
                        let new_pdpt = new_pdpt_frame.as_mut_ptr::<PageDirectoryPointer>();
                        unsafe { ptr::write_bytes(new_pdpt, 0, 1); }
                        
                        unsafe { (*new_pml4).entries[i].set(new_pdpt_frame, 0x03); }
                        self.clone_pdpt(i, new_pdpt)?;
                    }
                }
            },
            _ => {}
        }
        Ok(())
    }

    fn clone_pdpt(&self, pml4_idx: usize, new_pdpt: *mut PageDirectoryPointer) -> Result<(), VmmError> {
        let pdpt_entry = unsafe { &(*self.pml4).entries[pml4_idx] };
        let old_pdpt = PhysAddr(pdpt_entry.addr()).as_ptr::<PageDirectoryPointer>();
        
        for i in 0..512 {
            let entry = unsafe { &(*old_pdpt).entries[i] };
            if !entry.is_unused() && entry.addr() != 0 {
                let new_pd_frame = alloc_pages(1).ok_or(VmmError::OutOfMemory)?;
                let new_pd = new_pd_frame.as_mut_ptr::<PageDirectory>();
                unsafe { ptr::write_bytes(new_pd, 0, 1); }
                
                unsafe { (*new_pdpt).entries[i].set(new_pd_frame, 0x03); }
                self.clone_pd(i, new_pd)?;
            }
        }
        Ok(())
    }

    fn clone_pd(&self, pdpt_idx: usize, new_pd: *mut PageDirectory) -> Result<(), VmmError> {
        let pml4_idx = 0;
        let pdpt_entry = unsafe { &(*self.pml4).entries[pml4_idx] };
        let pdpt = PhysAddr(pdpt_entry.addr()).as_ptr::<PageDirectoryPointer>();
        let old_pd_entry = unsafe { &(*pdpt).entries[pdpt_idx] };
        let old_pd = PhysAddr(old_pd_entry.addr()).as_ptr::<PageDirectory>();
        
        for i in 0..512 {
            let entry = unsafe { &(*old_pd).entries[i] };
            if !entry.is_unused() && entry.addr() != 0 {
                let new_pt_frame = alloc_pages(1).ok_or(VmmError::OutOfMemory)?;
                let new_pt = new_pt_frame.as_mut_ptr::<PageTable>();
                unsafe { ptr::write_bytes(new_pt, 0, 1); }
                
                unsafe { (*new_pd).entries[i].set(new_pt_frame, 0x03); }
                self.clone_pt(i, new_pt)?;
            }
        }
        Ok(())
    }

    fn clone_pt(&self, pd_idx: usize, new_pt: *mut PageTable) -> Result<(), VmmError> {
        let pml4_idx = 0;
        let pdpt_idx = 0;
        let pdpt_entry = unsafe { &(*self.pml4).entries[pml4_idx] };
        let pdpt = PhysAddr(pdpt_entry.addr()).as_ptr::<PageDirectoryPointer>();
        let pd_entry = unsafe { &(*pdpt).entries[pdpt_idx] };
        let pd = PhysAddr(pd_entry.addr()).as_ptr::<PageDirectory>();
        let pt_entry = unsafe { &(*pd).entries[pd_idx] };
        let old_pt = PhysAddr(pt_entry.addr()).as_ptr::<PageTable>();
        
        for i in 0..512 {
            let entry = unsafe { &(*old_pt).entries[i] };
            if !entry.is_unused() && entry.addr() != 0 {
                let phys_addr = PhysAddr(entry.addr());
                self.mark_page_cow(VirtAddr(((pml4_idx << 39) | (pdpt_idx << 30) | (pd_idx << 21) | (i << 12)) as u64));
                unsafe { (*new_pt).entries[i].set(phys_addr, entry.flags() & !0x01); }
            }
        }
        Ok(())
    }

    pub fn protect_memory(&mut self, start: VirtAddr, size: usize, flags: u64) -> Result<(), VmmError> {
        let pages = (size + 0xFFF) / 0x1000;
        for i in 0..pages {
            let vaddr = VirtAddr(start.0 + (i as u64) * 0x1000);
            let pml4_index = (vaddr.0 >> 39) & 0x1FF;
            let pdpt_index = (vaddr.0 >> 30) & 0x1FF;
            let pd_index = (vaddr.0 >> 21) & 0x1FF;
            let pt_index = (vaddr.0 >> 12) & 0x1FF;

            let pdpt_entry = unsafe { &mut (*self.pml4).entries[pml4_index] };
            if pdpt_entry.is_unused() { continue; }
            let pdpt = PhysAddr(pdpt_entry.addr()).as_mut_ptr::<PageDirectoryPointer>();

            let pd_entry = unsafe { &mut (*pdpt).entries[pdpt_index] };
            if pd_entry.is_unused() { continue; }
            let pd = PhysAddr(pd_entry.addr()).as_mut_ptr::<PageDirectory>();

            let pt_entry = unsafe { &mut (*pd).entries[pd_index] };
            if pt_entry.is_unused() { continue; }
            let pt = PhysAddr(pt_entry.addr()).as_mut_ptr::<PageTable>();

            let pte = unsafe { &mut (*pt).entries[pt_index] };
            if !pte.is_unused() {
                pte.update_flags(flags);
            }
        }
        Ok(())
    }

    pub fn get_page_info(&self, virt: VirtAddr) -> Option<u64> {
        self.translate(virt).map(|phys| phys.0)
    }

    pub fn set_brk(&self, addr: VirtAddr) -> Result<VirtAddr, VmmError> {
        if addr.0 < self.heap_start.0 || addr.0 > self.heap_end.0 {
            return Err(VmmError::InvalidAddress);
        }
        self.brk.store(addr.0 as *mut u8, Ordering::SeqCst);
        Ok(addr)
    }

    pub fn get_brk(&self) -> VirtAddr {
        VirtAddr(self.brk.load(Ordering::SeqCst) as u64)
    }

    pub fn mmap(&mut self, addr: VirtAddr, length: usize, prot: u64, flags: u64) -> Result<VirtAddr, VmmError> {
        let pages = (length + 0xFFF) / 0x1000;
        
        if addr.0 != 0 {
            if !self.validate_user_range(addr, length) {
                return Err(VmmError::InvalidAddress);
            }
        } else {
            let mapped_addr = self.find_free_virtual_range(pages)?;
            for i in 0..pages {
                let vaddr = VirtAddr(mapped_addr.0 + (i as u64) * 0x1000);
                let frame = alloc_pages(1).ok_or(VmmError::OutOfMemory)?;
                self.map_page(vaddr, frame, prot)?;
            }
            return Ok(mapped_addr);
        }
        
        for i in 0..pages {
            let vaddr = VirtAddr(addr.0 + (i as u64) * 0x1000);
            let frame = alloc_pages(1).ok_or(VmmError::OutOfMemory)?;
            self.map_page(vaddr, frame, prot)?;
        }
        Ok(addr)
    }

    pub fn munmap(&mut self, addr: VirtAddr, length: usize) -> Result<(), VmmError> {
        let pages = (length + 0xFFF) / 0x1000;
        for i in 0..pages {
            let vaddr = VirtAddr(addr.0 + (i as u64) * 0x1000);
            self.unmap_page(vaddr)?;
        }
        Ok(())
    }

    pub fn verify_access(&self, addr: VirtAddr, size: usize, write: bool) -> bool {
        if !self.validate_user_range(addr, size) {
            return false;
        }
        for i in 0..size {
            let vaddr = VirtAddr(addr.0 + i as u64);
            if self.translate(vaddr).is_none() {
                return false;
            }
        }
        true
    }

    pub fn get_memory_usage(&self) -> usize {
        self.page_count.load(Ordering::SeqCst) * 0x1000
    }

    pub fn map_kernel_page(&mut self, virt: VirtAddr, phys: PhysAddr, flags: u64) -> Result<(), VmmError> {
        let pml4_index = (virt.0 >> 39) & 0x1FF;
        let pdpt_index = (virt.0 >> 30) & 0x1FF;
        let pd_index = (virt.0 >> 21) & 0x1FF;
        let pt_index = (virt.0 >> 12) & 0x1FF;

        let pdpt_entry = unsafe { &mut (*self.pml4).entries[pml4_index] };
        if pdpt_entry.is_unused() {
            let frame = alloc_pages(1).ok_or(VmmError::OutOfMemory)?;
            pdpt_entry.set(frame, 0x03);
            self.page_count.fetch_add(1, Ordering::SeqCst);
        }
        let pdpt = PhysAddr(pdpt_entry.addr()).as_mut_ptr::<PageDirectoryPointer>();

        let pd_entry = unsafe { &mut (*pdpt).entries[pdpt_index] };
        if pd_entry.is_unused() {
            let frame = alloc_pages(1).ok_or(VmmError::OutOfMemory)?;
            pd_entry.set(frame, 0x03);
            self.page_count.fetch_add(1, Ordering::SeqCst);
        }
        let pd = PhysAddr(pd_entry.addr()).as_mut_ptr::<PageDirectory>();

        let pt_entry = unsafe { &mut (*pd).entries[pd_index] };
        if pt_entry.is_unused() {
            let frame = alloc_pages(1).ok_or(VmmError::OutOfMemory)?;
            pt_entry.set(frame, 0x03);
            self.page_count.fetch_add(1, Ordering::SeqCst);
        }
        let pt = PhysAddr(pt_entry.addr()).as_mut_ptr::<PageTable>();

        let pte = unsafe { &mut (*pt).entries[pt_index] };
        pte.set(phys, flags);
        self.page_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct VirtAddr(pub u64);

impl VirtAddr {
    pub fn align_down(&self, align: u64) -> Self {
        VirtAddr(self.0 & !(align - 1))
    }

    pub fn align_up(&self, align: u64) -> Self {
        VirtAddr((self.0 + align - 1) & !(align - 1))
    }
}

