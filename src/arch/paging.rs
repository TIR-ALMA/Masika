use core::arch::asm;
use core::ptr;

const PAGE_SIZE: usize = 4096;
const ENTRY_COUNT: usize = 512;

#[derive(Debug)]
pub struct PageTable {
    pub entries: [PageTableEntry; ENTRY_COUNT],
}

impl PageTable {
    pub const fn new() -> Self {
        Self { entries: [PageTableEntry::empty(); ENTRY_COUNT] }
    }

    pub fn get_entry(&self, index: usize) -> Option<&PageTableEntry> {
        self.entries.get(index)
    }

    pub fn get_entry_mut(&mut self, index: usize) -> Option<&mut PageTableEntry> {
        self.entries.get_mut(index)
    }

    pub fn set_entry(&mut self, index: usize, entry: PageTableEntry) {
        if let Some(e) = self.entries.get_mut(index) {
            *e = entry;
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub const fn empty() -> Self {
        PageTableEntry(0)
    }

    pub fn present(&self) -> bool {
        self.0 & 1 != 0
    }

    pub fn writable(&self) -> bool {
        self.0 & 2 != 0
    }

    pub fn user_accessible(&self) -> bool {
        self.0 & 4 != 0
    }

    pub fn write_through(&self) -> bool {
        self.0 & 8 != 0
    }

    pub fn cache_disabled(&self) -> bool {
        self.0 & 16 != 0
    }

    pub fn accessed(&self) -> bool {
        self.0 & 32 != 0
    }

    pub fn dirty(&self) -> bool {
        self.0 & 64 != 0
    }

    pub fn huge_page(&self) -> bool {
        self.0 & 128 != 0
    }

    pub fn global(&self) -> bool {
        self.0 & 256 != 0
    }

    pub fn addr(&self) -> u64 {
        self.0 & 0x000FFFF_FFFFFFF0
    }

    pub fn set_addr(&mut self, addr: u64) {
        self.0 = (self.0 & !0x000FFFF_FFFFFFF0) | (addr & 0x000FFFF_FFFFFFF0);
    }

    pub fn set_present(&mut self, present: bool) {
        self.0 = (self.0 & !1) | (present as u64);
    }

    pub fn set_writable(&mut self, writable: bool) {
        self.0 = (self.0 & !2) | ((writable as u64) << 1);
    }

    pub fn set_user(&mut self, user: bool) {
        self.0 = (self.0 & !4) | ((user as u64) << 2);
    }

    pub fn set_write_through(&mut self, wt: bool) {
        self.0 = (self.0 & !8) | ((wt as u64) << 3);
    }

    pub fn set_cache_disabled(&mut self, cd: bool) {
        self.0 = (self.0 & !16) | ((cd as u64) << 4);
    }

    pub fn set_accessed(&mut self, accessed: bool) {
        self.0 = (self.0 & !32) | ((accessed as u64) << 5);
    }

    pub fn set_dirty(&mut self, dirty: bool) {
        self.0 = (self.0 & !64) | ((dirty as u64) << 6);
    }

    pub fn set_huge(&mut self, huge: bool) {
        self.0 = (self.0 & !128) | ((huge as u64) << 7);
    }

    pub fn set_global(&mut self, global: bool) {
        self.0 = (self.0 & !256) | ((global as u64) << 8);
    }

    pub fn flags(&self) -> u64 {
        self.0 & 0xFFF
    }

    pub fn set_flags(&mut self, flags: u64) {
        self.0 = (self.0 & !0xFFF) | (flags & 0xFFF);
    }
}

#[repr(C, align(4096))]
pub struct AlignedPageTable {
    inner: PageTable,
}

impl AlignedPageTable {
    pub const fn new() -> Self {
        Self { inner: PageTable::new() }
    }

    pub fn as_ptr(&self) -> *const PageTable {
        &self.inner as *const _
    }

    pub fn as_mut_ptr(&mut self) -> *mut PageTable {
        &mut self.inner as *mut _
    }
}

static mut KERNEL_PML4: AlignedPageTable = AlignedPageTable::new();

pub fn init_paging() {
    unsafe {
        let pml4_addr = KERNEL_PML4.as_ptr() as u64;
        asm!("mov cr3, {}", in(reg) pml4_addr, options(nostack));
    }

    enable_nxe_bit();
    enable_write_protect();
}

fn enable_nxe_bit() {
    unsafe {
        let mut efer: u64;
        asm!("rdmsr", out("eax") efer, out("edx") _, const 0xC0000080, options(nomem, nostack));
        efer |= 1 << 11;
        asm!("wrmsr", in("eax") efer as u32, in("edx") (efer >> 32) as u32, const 0xC0000080, options(nomem, nostack));
    }
}

fn enable_write_protect() {
    unsafe {
        let mut cr0: u64;
        asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack));
        cr0 |= 1 << 16;
        asm!("mov cr0, {}", in(reg) cr0, options(nomem, nostack));
    }
}

pub fn map_page(virt: u64, phys: u64, flags: u64) {
    let pml4_index = (virt >> 39) & 0x1FF;
    let pdpt_index = (virt >> 30) & 0x1FF;
    let pd_index = (virt >> 21) & 0x1FF;
    let pt_index = (virt >> 12) & 0x1FF;

    unsafe {
        let pml4 = &mut *(KERNEL_PML4.as_mut_ptr());
        let pml4_entry = &mut pml4.entries[pml4_index as usize];
        if !pml4_entry.present() {
            let pdpt = Box::leak(Box::new(PageTable::new()));
            let pdpt_addr = pdpt as *mut _ as u64;
            pml4_entry.set_addr(pdpt_addr);
            pml4_entry.set_present(true);
            pml4_entry.set_writable(true);
        }

        let pdpt = &mut *(pml4_entry.addr() as *mut PageTable);
        let pdpt_entry = &mut pdpt.entries[pdpt_index as usize];
        if !pdpt_entry.present() {
            let pd = Box::leak(Box::new(PageTable::new()));
            let pd_addr = pd as *mut _ as u64;
            pdpt_entry.set_addr(pd_addr);
            pdpt_entry.set_present(true);
            pdpt_entry.set_writable(true);
        }

        let pd = &mut *(pdpt_entry.addr() as *mut PageTable);
        let pd_entry = &mut pd.entries[pd_index as usize];
        if !pd_entry.present() {
            let pt = Box::leak(Box::new(PageTable::new()));
            let pt_addr = pt as *mut _ as u64;
            pd_entry.set_addr(pt_addr);
            pd_entry.set_present(true);
            pd_entry.set_writable(true);
        }

        let pt = &mut *(pd_entry.addr() as *mut PageTable);
        let pt_entry = &mut pt.entries[pt_index as usize];
        pt_entry.set_addr(phys);
        pt_entry.set_flags(flags);
    }
}

pub fn unmap_page(virt: u64) {
    let pml4_index = (virt >> 39) & 0x1FF;
    let pdpt_index = (virt >> 30) & 0x1FF;
    let pd_index = (virt >> 21) & 0x1FF;
    let pt_index = (virt >> 12) & 0x1FF;

    unsafe {
        let pml4 = &mut *(KERNEL_PML4.as_mut_ptr());
        let pml4_entry = &pml4.entries[pml4_index as usize];
        if !pml4_entry.present() {
            return;
        }

        let pdpt = &mut *(pml4_entry.addr() as *mut PageTable);
        let pdpt_entry = &pdpt.entries[pdpt_index as usize];
        if !pdpt_entry.present() {
            return;
        }

        let pd = &mut *(pdpt_entry.addr() as *mut PageTable);
        let pd_entry = &pd.entries[pd_index as usize];
        if !pd_entry.present() {
            return;
        }

        let pt = &mut *(pd_entry.addr() as *mut PageTable);
        let pt_entry = &mut pt.entries[pt_index as usize];
        pt_entry.set_flags(0);
        flush_tlb_single(virt);
    }
}

pub fn translate_virt_to_phys(virt: u64) -> Option<u64> {
    let pml4_index = (virt >> 39) & 0x1FF;
    let pdpt_index = (virt >> 30) & 0x1FF;
    let pd_index = (virt >> 21) & 0x1FF;
    let pt_index = (virt >> 12) & 0x1FF;

    unsafe {
        let pml4 = &*(KERNEL_PML4.as_ptr());
        let pml4_entry = &pml4.entries[pml4_index as usize];
        if !pml4_entry.present() {
            return None;
        }

        let pdpt = &*(pml4_entry.addr() as *const PageTable);
        let pdpt_entry = &pdpt.entries[pdpt_index as usize];
        if !pdpt_entry.present() {
            return None;
        }

        let pd = &*(pdpt_entry.addr() as *const PageTable);
        let pd_entry = &pd.entries[pd_index as usize];
        if !pd_entry.present() {
            return None;
        }

        let pt = &*(pd_entry.addr() as *const PageTable);
        let pt_entry = &pt.entries[pt_index as usize];
        if !pt_entry.present() {
            return None;
        }

        Some((pt_entry.addr() & !0xFFF) | (virt & 0xFFF))
    }
}

pub fn flush_tlb() {
    unsafe {
        let cr3_val: u64;
        asm!("mov {}, cr3", out(reg) cr3_val, options(nomem, nostack));
        asm!("mov cr3, {}", in(reg) cr3_val, options(nomem, nostack));
    }
}

pub fn flush_tlb_single(addr: u64) {
    unsafe {
        asm!("invlpg [{}]", in(reg) addr, options(nostack));
    }
}

pub fn set_kernel_page_table() {
    unsafe {
        let pml4_addr = KERNEL_PML4.as_ptr() as u64;
        asm!("mov cr3, {}", in(reg) pml4_addr, options(nostack));
    }
}

pub fn get_current_pml4() -> u64 {
    let cr3_val: u64;
    unsafe {
        asm!("mov {}, cr3", out(reg) cr3_val, options(nomem, nostack));
    }
    cr3_val
}

pub fn clone_page_table() -> u64 {
    unsafe {
        let old_pml4 = get_current_pml4() as *const PageTable;
        let new_pml4 = Box::into_raw(Box::new(PageTable::new()));
        let new_pml4_ptr = new_pml4 as u64;

        for i in 0..ENTRY_COUNT {
            let entry = (*old_pml4).entries[i];
            if entry.present() && !entry.huge_page() {
                let cloned_pt = clone_pt_or_pd(entry.addr());
                (*new_pml4).set_entry(i, PageTableEntry(cloned_pt.0));
            } else {
                (*new_pml4).set_entry(i, entry);
            }
        }

        new_pml4_ptr
    }
}

unsafe fn clone_pt_or_pd(old_addr: u64) -> PageTableEntry {
    let old_pt = old_addr as *const PageTable;
    let new_pt = Box::into_raw(Box::new(PageTable::new()));
    let new_pt_ptr = new_pt as u64;

    for j in 0..ENTRY_COUNT {
        let entry = (*old_pt).entries[j];
        (*new_pt).set_entry(j, entry);
    }

    let mut new_entry = PageTableEntry(0);
    new_entry.set_addr(new_pt_ptr);
    new_entry.set_present((*old_pt).entries[0].present());
    new_entry.set_writable((*old_pt).entries[0].writable());
    new_entry.set_user((*old_pt).entries[0].user_accessible());
    new_entry
}

pub fn map_range(virt_start: u64, phys_start: u64, size: usize, flags: u64) {
    let pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;
    for i in 0..pages {
        let v = virt_start + (i * PAGE_SIZE) as u64;
        let p = phys_start + (i * PAGE_SIZE) as u64;
        map_page(v, p, flags);
    }
}

pub fn unmap_range(virt_start: u64, size: usize) {
    let pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;
    for i in 0..pages {
        let v = virt_start + (i * PAGE_SIZE) as u64;
        unmap_page(v);
    }
}

pub fn alloc_and_map_kernel_page(virt: u64, flags: u64) -> Option<u64> {
    let phys = crate::memory::physical::allocate_frame()?;
    map_page(virt, phys, flags);
    Some(phys)
}

pub fn alloc_and_map_user_page(virt: u64, flags: u64) -> Option<u64> {
    let phys = crate::memory::physical::allocate_frame()?;
    let user_flags = flags | 4; // Add user bit
    map_page(virt, phys, user_flags);
    Some(phys)
}

pub fn is_page_mapped(virt: u64) -> bool {
    translate_virt_to_phys(virt).is_some()
}

pub fn ensure_page_mapped(virt: u64, flags: u64) -> bool {
    if is_page_mapped(virt) {
        return true;
    }
    if let Some(_) = alloc_and_map_kernel_page(virt, flags) {
        true
    } else {
        false
    }
}

pub fn get_page_flags(virt: u64) -> Option<u64> {
    let pml4_index = (virt >> 39) & 0x1FF;
    let pdpt_index = (virt >> 30) & 0x1FF;
    let pd_index = (virt >> 21) & 0x1FF;
    let pt_index = (virt >> 12) & 0x1FF;

    unsafe {
        let pml4 = &*(KERNEL_PML4.as_ptr());
        let pml4_entry = &pml4.entries[pml4_index as usize];
        if !pml4_entry.present() { return None; }

        let pdpt = &*(pml4_entry.addr() as *const PageTable);
        let pdpt_entry = &pdpt.entries[pdpt_index as usize];
        if !pdpt_entry.present() { return None; }

        let pd = &*(pdpt_entry.addr() as *const PageTable);
        let pd_entry = &pd.entries[pd_index as usize];
        if !pd_entry.present() { return None; }

        let pt = &*(pd_entry.addr() as *const PageTable);
        let pt_entry = &pt.entries[pt_index as usize];
        if !pt_entry.present() { return None; }

        Some(pt_entry.flags())
    }
}

pub fn set_page_flags(virt: u64, flags: u64) {
    let pml4_index = (virt >> 39) & 0x1FF;
    let pdpt_index = (virt >> 30) & 0x1FF;
    let pd_index = (virt >> 21) & 0x1FF;
    let pt_index = (virt >> 12) & 0x1FF;

    unsafe {
        let pml4 = &mut *(KERNEL_PML4.as_mut_ptr());
        let pml4_entry = &mut pml4.entries[pml4_index as usize];
        if !pml4_entry.present() { return; }

        let pdpt = &mut *(pml4_entry.addr() as *mut PageTable);
        let pdpt_entry = &mut pdpt.entries[pdpt_index as usize];
        if !pdpt_entry.present() { return; }

        let pd = &mut *(pdpt_entry.addr() as *mut PageTable);
        let pd_entry = &mut pd.entries[pd_index as usize];
        if !pd_entry.present() { return; }

        let pt = &mut *(pd_entry.addr() as *mut PageTable);
        let pt_entry = &mut pt.entries[pt_index as usize];
        if !pt_entry.present() { return; }

        pt_entry.set_flags(flags);
        flush_tlb_single(virt);
    }
}

pub fn identity_map_frame(phys: u64) {
    map_page(phys, phys, 3); // Present + Writable
}

pub fn map_kernel_space(start: u64, end: u64, flags: u64) {
    let mut addr = start;
    while addr < end {
        map_page(addr, addr, flags);
        addr += PAGE_SIZE as u64;
    }
}

pub fn remap_page(virt: u64, new_phys: u64, flags: u64) {
    unmap_page(virt);
    map_page(virt, new_phys, flags);
}

pub fn copy_page(src_virt: u64, dst_virt: u64) {
    if let Some(src_phys) = translate_virt_to_phys(src_virt) {
        if let Some(dst_phys) = translate_virt_to_phys(dst_virt) {
            unsafe {
                ptr::copy_nonoverlapping(
                    src_phys as *const u8,
                    dst_phys as *mut u8,
                    PAGE_SIZE,
                );
            }
        }
    }
}

pub fn mark_page_accessed(virt: u64) {
    let pml4_index = (virt >> 39) & 0x1FF;
    let pdpt_index = (virt >> 30) & 0x1FF;
    let pd_index = (virt >> 21) & 0x1FF;
    let pt_index = (virt >> 12) & 0x1FF;

    unsafe {
        let pml4 = &mut *(KERNEL_PML4.as_mut_ptr());
        let pml4_entry = &mut pml4.entries[pml4_index as usize];
        if !pml4_entry.present() { return; }

        let pdpt = &mut *(pml4_entry.addr() as *mut PageTable);
        let pdpt_entry = &mut pdpt.entries[pdpt_index as usize];
        if !pdpt_entry.present() { return; }

        let pd = &mut *(pdpt_entry.addr() as *mut PageTable);
        let pd_entry = &mut pd.entries[pd_index as usize];
        if !pd_entry.present() { return; }

        let pt = &mut *(pd_entry.addr() as *mut PageTable);
        let pt_entry = &mut pt.entries[pt_index as usize];
        if !pt_entry.present() { return; }

        pt_entry.set_accessed(true);
    }
}

pub fn mark_page_dirty(virt: u64) {
    let pml4_index = (virt >> 39) & 0x1FF;
    let pdpt_index = (virt >> 30) & 0x1FF;
    let pd_index = (virt >> 21) & 0x1FF;
    let pt_index = (virt >> 12) & 0x1FF;

    unsafe {
        let pml4 = &mut *(KERNEL_PML4.as_mut_ptr());
        let pml4_entry = &mut pml4.entries[pml4_index as usize];
        if !pml4_entry.present() { return; }

        let pdpt = &mut *(pml4_entry.addr() as *mut PageTable);
        let pdpt_entry = &mut pdpt.entries[pdpt_index as usize];
        if !pdpt_entry.present() { return; }

        let pd = &mut *(pdpt_entry.addr() as *mut PageTable);
        let pd_entry = &mut pd.entries[pd_index as usize];
        if !pd_entry.present() { return; }

        let pt = &mut *(pd_entry.addr() as *mut PageTable);
        let pt_entry = &mut pt.entries[pt_index as usize];
        if !pt_entry.present() { return; }

        pt_entry.set_dirty(true);
    }
}

pub fn is_page_dirty(virt: u64) -> bool {
    if let Some(flags) = get_page_flags(virt) {
        (flags & 64) != 0
    } else {
        false
    }
}

pub fn is_page_accessed(virt: u64) -> bool {
    if let Some(flags) = get_page_flags(virt) {
        (flags & 32) != 0
    } else {
        false
    }
}

pub fn protect_page(virt: u64, writable: bool) {
    let flags = if writable { 3 } else { 1 }; // Present + Writable or just Present
    set_page_flags(virt, flags);
}

pub fn enable_4gb_paging() {
    unsafe {
        let mut cr4: u64;
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack));
        cr4 |= 1 << 7; // PGE
        asm!("mov cr4, {}", in(reg) cr4, options(nomem, nostack));
    }
}

pub fn disable_4gb_paging() {
    unsafe {
        let mut cr4: u64;
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack));
        cr4 &= !(1 << 7); // Clear PGE
        asm!("mov cr4, {}", in(reg) cr4, options(nomem, nostack));
    }
}

pub fn switch_page_table(pml4_addr: u64) {
    unsafe {
        asm!("mov cr3, {}", in(reg) pml4_addr, options(nostack));
    }
}

pub fn get_page_directory_addr(virt: u64) -> Option<u64> {
    let pml4_index = (virt >> 39) & 0x1FF;
    unsafe {
        let pml4 = &*(KERNEL_PML4.as_ptr());
        let pml4_entry = &pml4.entries[pml4_index as usize];
        if !pml4_entry.present() { return None; }
        Some(pml4_entry.addr())
    }
}

pub fn get_page_table_addr(virt: u64) -> Option<u64> {
    let pml4_index = (virt >> 39) & 0x1FF;
    let pdpt_index = (virt >> 30) & 0x1FF;
    unsafe {
        let pml4 = &*(KERNEL_PML4.as_ptr());
        let pml4_entry = &pml4.entries[pml4_index as usize];
        if !pml4_entry.present() { return None; }

        let pdpt = &*(pml4_entry.addr() as *const PageTable);
        let pdpt_entry = &pdpt.entries[pdpt_index as usize];
        if !pdpt_entry.present() { return None; }

        Some(pdpt_entry.addr())
