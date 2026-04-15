use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

const PAGE_SIZE: usize = 4096;
const FRAME_SIZE: usize = PAGE_SIZE;

pub struct PhysicalMemoryManager {
    bitmap: *mut u64,
    total_frames: usize,
    used_frames: AtomicUsize,
    reserved_frames: AtomicUsize,
    max_reserved_frame: AtomicUsize,
}

impl PhysicalMemoryManager {
    pub fn new(memory_size: usize, base_addr: usize) -> Self {
        let total_frames = memory_size / FRAME_SIZE;
        let bitmap_size = (total_frames + 63) / 64;
        let bitmap_ptr = unsafe { ptr::read_volatile((base_addr as *const usize) as *const *mut u8) as *mut u64 };
        let aligned_bitmap = ((bitmap_ptr as usize + 0xFFF) & !0xFFF) as *mut u64;
        Self {
            bitmap: aligned_bitmap,
            total_frames,
            used_frames: AtomicUsize::new(0),
            reserved_frames: AtomicUsize::new(0),
            max_reserved_frame: AtomicUsize::new(0),
        }
    }

    fn get_bit(&self, frame: usize) -> bool {
        if frame >= self.total_frames { return false; }
        let word_index = frame >> 6;
        let bit_index = frame & 63;
        unsafe { self.bitmap.add(word_index).read() & (1u64 << bit_index) != 0 }
    }

    fn set_bit(&self, frame: usize) {
        if frame >= self.total_frames { return; }
        let word_index = frame >> 6;
        let bit_index = frame & 63;
        unsafe {
            let addr = self.bitmap.add(word_index);
            let value = addr.read();
            addr.write(value | (1u64 << bit_index));
        }
    }

    fn clear_bit(&self, frame: usize) {
        if frame >= self.total_frames { return; }
        let word_index = frame >> 6;
        let bit_index = frame & 63;
        unsafe {
            let addr = self.bitmap.add(word_index);
            let value = addr.read();
            addr.write(value & !(1u64 << bit_index));
        }
    }

    fn find_first_free_frame_from(&self, start: usize) -> Option<usize> {
        let start_word = start >> 6;
        let start_bit = start & 63;
        
        for word_idx in start_word..((self.total_frames + 63) >> 6) {
            let word = unsafe { self.bitmap.add(word_idx).read() };
            let mask = if word_idx == start_word { !((1u64 << start_bit) - 1) } else { u64::MAX };
            let masked_word = !word & mask;
            
            if masked_word != 0 {
                let bit_pos = masked_word.trailing_zeros() as usize;
                let frame = (word_idx << 6) + bit_pos;
                if frame < self.total_frames {
                    return Some(frame);
                }
            }
        }
        None
    }

    pub fn alloc_frame(&self) -> Option<usize> {
        let start_hint = self.max_reserved_frame.load(Ordering::Relaxed);
        if let Some(frame) = self.find_first_free_frame_from(start_hint) {
            self.set_bit(frame);
            self.used_frames.fetch_add(1, Ordering::SeqCst);
            let max = self.max_reserved_frame.load(Ordering::Relaxed);
            if frame > max {
                self.max_reserved_frame.store(frame, Ordering::Relaxed);
            }
            return Some(frame * FRAME_SIZE);
        }
        
        for frame in 0..self.total_frames {
            if !self.get_bit(frame) {
                self.set_bit(frame);
                self.used_frames.fetch_add(1, Ordering::SeqCst);
                let max = self.max_reserved_frame.load(Ordering::Relaxed);
                if frame > max {
                    self.max_reserved_frame.store(frame, Ordering::Relaxed);
                }
                return Some(frame * FRAME_SIZE);
            }
        }
        None
    }

    pub fn alloc_frames_contiguous(&self, count: usize) -> Option<usize> {
        if count == 0 { return None; }
        if count == 1 { return self.alloc_frame(); }

        for start in 0..=(self.total_frames - count) {
            let mut free = true;
            for offset in 0..count {
                if self.get_bit(start + offset) {
                    free = false;
                    break;
                }
            }
            if free {
                for offset in 0..count {
                    self.set_bit(start + offset);
                }
                self.used_frames.fetch_add(count, Ordering::SeqCst);
                let max = self.max_reserved_frame.load(Ordering::Relaxed);
                if start + count > max {
                    self.max_reserved_frame.store(start + count, Ordering::Relaxed);
                }
                return Some(start * FRAME_SIZE);
            }
        }
        None
    }

    pub fn free_frame(&self, addr: usize) {
        let frame = addr / FRAME_SIZE;
        if frame < self.total_frames && self.get_bit(frame) {
            self.clear_bit(frame);
            self.used_frames.fetch_sub(1, Ordering::SeqCst);
        }
    }

    pub fn reserve_frame(&self, addr: usize) {
        let frame = addr / FRAME_SIZE;
        if frame < self.total_frames && !self.get_bit(frame) {
            self.set_bit(frame);
            self.reserved_frames.fetch_add(1, Ordering::SeqCst);
            let max = self.max_reserved_frame.load(Ordering::Relaxed);
            if frame > max {
                self.max_reserved_frame.store(frame, Ordering::Relaxed);
            }
        }
    }

    pub fn free_frames_contiguous(&self, addr: usize, count: usize) {
        let start_frame = addr / FRAME_SIZE;
        for i in 0..count {
            let frame = start_frame + i;
            if frame < self.total_frames && self.get_bit(frame) {
                self.clear_bit(frame);
                self.used_frames.fetch_sub(1, Ordering::SeqCst);
            }
        }
    }

    pub fn get_used_frames(&self) -> usize {
        self.used_frames.load(Ordering::SeqCst)
    }

    pub fn get_free_frames(&self) -> usize {
        self.total_frames - self.used_frames.load(Ordering::SeqCst) - self.reserved_frames.load(Ordering::SeqCst)
    }

    pub fn is_frame_free(&self, addr: usize) -> bool {
        let frame = addr / FRAME_SIZE;
        !self.get_bit(frame)
    }

    pub fn alloc_n_frames(&self, n: usize) -> Option<*mut u8> {
        if n == 1 {
            self.alloc_frame().map(|addr| addr as *mut u8)
        } else {
            self.alloc_frames_contiguous(n).map(|addr| addr as *mut u8)
        }
    }

    pub fn free_n_frames(&self, ptr: *mut u8, n: usize) {
        let addr = ptr as usize;
        if n == 1 {
            self.free_frame(addr);
        } else {
            self.free_frames_contiguous(addr, n);
        }
    }

    pub fn init_region(&self, base: usize, size: usize) {
        let start_frame = base / FRAME_SIZE;
        let end_frame = (base + size + FRAME_SIZE - 1) / FRAME_SIZE;
        for i in start_frame..end_frame.min(self.total_frames) {
            self.clear_bit(i);
        }
    }

    pub fn mark_as_used(&self, base: usize, size: usize) {
        let start_frame = base / FRAME_SIZE;
        let end_frame = (base + size + FRAME_SIZE - 1) / FRAME_SIZE;
        for i in start_frame..end_frame.min(self.total_frames) {
            self.set_bit(i);
        }
        self.used_frames.fetch_add(end_frame - start_frame, Ordering::SeqCst);
    }

    pub fn get_total_memory(&self) -> usize {
        self.total_frames * FRAME_SIZE
    }

    pub fn alloc_at(&self, addr: usize) -> Result<(), ()> {
        let frame = addr / FRAME_SIZE;
        if frame >= self.total_frames || self.get_bit(frame) {
            Err(())
        } else {
            self.set_bit(frame);
            self.used_frames.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    pub fn validate_range(&self, base: usize, size: usize) -> bool {
        let start_frame = base / FRAME_SIZE;
        let end_frame = (base + size + FRAME_SIZE - 1) / FRAME_SIZE;
        if end_frame > self.total_frames {
            return false;
        }
        for i in start_frame..end_frame {
            if self.get_bit(i) {
                return false;
            }
        }
        true
    }

    pub fn zero_frame(&self, addr: usize) {
        unsafe {
            ptr::write_bytes(addr as *mut u8, 0, FRAME_SIZE);
        }
    }

    pub fn zero_frames(&self, addr: usize, count: usize) {
        unsafe {
            ptr::write_bytes(addr as *mut u8, 0, count * FRAME_SIZE);
        }
    }

    pub fn get_frame_count(&self) -> usize {
        self.total_frames
    }

    pub fn alloc_zeroed_frame(&self) -> Option<usize> {
        if let Some(addr) = self.alloc_frame() {
            self.zero_frame(addr);
            Some(addr)
        } else {
            None
        }
    }

    pub fn alloc_zeroed_frames_contiguous(&self, count: usize) -> Option<usize> {
        if let Some(addr) = self.alloc_frames_contiguous(count) {
            self.zero_frames(addr, count);
            Some(addr)
        } else {
            None
        }
    }

    pub fn reclaim_frame(&self, addr: usize) {
        self.reserve_frame(addr);
    }

    pub fn release_frame(&self, addr: usize) {
        self.free_frame(addr);
    }

    pub fn alloc_aligned(&self, align: usize, size: usize) -> Option<usize> {
        let frames_needed = (size + FRAME_SIZE - 1) / FRAME_SIZE;
        let align_frames = align / FRAME_SIZE;
        for start in (0..self.total_frames).step_by(align_frames.max(1)) {
            let mut free = true;
            for offset in 0..frames_needed {
                if start + offset >= self.total_frames || self.get_bit(start + offset) {
                    free = false;
                    break;
                }
            }
            if free {
                for offset in 0..frames_needed {
                    self.set_bit(start + offset);
                }
                self.used_frames.fetch_add(frames_needed, Ordering::SeqCst);
                return Some(start * FRAME_SIZE);
            }
        }
        None
    }

    pub fn alloc_huge_page(&self) -> Option<usize> {
        self.alloc_frames_contiguous(512)
    }

    pub fn free_huge_page(&self, addr: usize) {
        self.free_frames_contiguous(addr, 512);
    }

    pub fn get_bitmap_ptr(&self) -> *const u64 {
        self.bitmap
    }

    pub fn dump_stats(&self) {
        let used = self.get_used_frames();
        let total = self.get_total_memory();
        let free = total - used * FRAME_SIZE;
    }

    pub fn alloc_multiple_frames(&self, count: usize) -> Option<usize> {
        if count <= 1 {
            return self.alloc_frame();
        }
        self.alloc_frames_contiguous(count)
    }

    pub fn get_reserved_frames(&self) -> usize {
        self.reserved_frames.load(Ordering::SeqCst)
    }

    pub fn get_max_frame(&self) -> usize {
        self.total_frames
    }

    pub fn alloc_specific_frame(&self, frame: usize) -> Result<(), ()> {
        if frame >= self.total_frames || self.get_bit(frame) {
            return Err(());
        }
        self.set_bit(frame);
        self.used_frames.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub fn get_frame_address(&self, frame: usize) -> Option<usize> {
        if frame < self.total_frames {
            Some(frame * FRAME_SIZE)
        } else {
            None
        }
    }

    pub fn is_valid_frame(&self, addr: usize) -> bool {
        let frame = addr / FRAME_SIZE;
        frame < self.total_frames
    }

    pub fn alloc_kernel_heap_space(&self, size: usize) -> Option<usize> {
        let pages_needed = (size + FRAME_SIZE - 1) / FRAME_SIZE;
        self.alloc_frames_contiguous(pages_needed)
    }

    pub fn get_usage_percentage(&self) -> f32 {
        let used = self.get_used_frames();
        let total = self.total_frames;
        if total == 0 { 0.0 } else { (used as f32 / total as f32) * 100.0 }
    }

    pub fn find_largest_free_block(&self) -> usize {
        let mut max_free = 0;
        let mut current_free = 0;

        for i in 0..self.total_frames {
            if !self.get_bit(i) {
                current_free += 1;
            } else {
                if current_free > max_free {
                    max_free = current_free;
                }
                current_free = 0;
            }
        }
        if current_free > max_free {
            max_free = current_free;
        }
        max_free
    }

    pub fn get_largest_contiguous_allocatable(&self) -> usize {
        self.find_largest_free_block()
    }

    pub fn alloc_specific_range(&self, start_addr: usize, end_addr: usize) -> Result<(), ()> {
        let start_frame = start_addr / FRAME_SIZE;
        let end_frame = (end_addr + FRAME_SIZE - 1) / FRAME_SIZE;
        
        for frame in start_frame..end_frame {
            if frame >= self.total_frames || self.get_bit(frame) {
                return Err(());
            }
        }

        for frame in start_frame..end_frame {
            self.set_bit(frame);
        }
        self.used_frames.fetch_add(end_frame - start_frame, Ordering::SeqCst);
        Ok(())
    }

    pub fn is_frame_allocated(&self, addr: usize) -> bool {
        let frame = addr / FRAME_SIZE;
        self.get_bit(frame)
    }

    pub fn alloc_and_map(&self, size: usize, vmm: &dyn Fn(usize, usize) -> bool) -> Option<usize> {
        let frames_needed = (size + FRAME_SIZE - 1) / FRAME_SIZE;
        if let Some(paddr) = self.alloc_frames_contiguous(frames_needed) {
            if vmm(paddr, size) {
                return Some(paddr);
            } else {
                self.free_frames_contiguous(paddr, frames_needed);
            }
        }
        None
    }

    pub fn reserve_region_for_boot(&self, base: usize, size: usize) {
        let start_frame = base / FRAME_SIZE;
        let end_frame = (base + size + FRAME_SIZE - 1) / FRAME_SIZE;
        for i in start_frame..end_frame.min(self.total_frames) {
            self.set_bit(i);
        }
        self.reserved_frames.fetch_add(end_frame - start_frame, Ordering::SeqCst);
    }

    pub fn get_available_memory(&self) -> usize {
        self.get_free_frames() * FRAME_SIZE
    }

    pub fn alloc_dma_capable_frame(&self) -> Option<usize> {
        if let Some(frame) = self.alloc_frame() {
            if frame < 0x100000 {
                Some(frame)
            } else {
                self.free_frame(frame);
                None
            }
        } else {
            None
        }
    }

    pub fn is_low_memory(&self) -> bool {
        self.get_free_frames() < 1024
    }

    pub fn alloc_guarded_region(&self, size: usize) -> Option<(usize, usize, usize)> {
        let size_frames = (size + FRAME_SIZE - 1) / FRAME_SIZE;
        if let Some(region) = self.alloc_frames_contiguous(size_frames + 2) {
            Some((region + FRAME_SIZE, size, region))
        } else {
            None
        }
    }

    pub fn free_guarded_region(&self, addr: usize, size: usize, guard_before: usize) {
        let size_frames = (size + FRAME_SIZE - 1) / FRAME_SIZE;
        self.free_frames_contiguous(guard_before, size_frames + 2);
    }

    pub fn get_memory_map(&self) -> (usize, usize, usize) {
        (self.get_used_frames(), self.get_reserved_frames(), self.get_free_frames())
    }

    pub fn compact_bitmap(&self) {
        // No-op for this implementation
    }

    pub fn set_bitmap_byte(&self, byte_index: usize, value: u8) {
        let word_idx = byte_index / 8;
        let bit_shift = (byte_index % 8) * 8;
        let mask = (value as u64) << bit_shift;
        unsafe {
            let addr = self.bitmap.add(word_idx);
            let old = addr.read();
            addr.write((old & !(0xFFu64 << bit_shift)) | mask);
        }
    }

    pub fn get_physical_address(&self, frame: usize) -> Option<usize> {
        if frame < self.total_frames {
            Some(frame * FRAME_SIZE)
        } else {
            None
        }
    }

    pub fn is_frame_reserved(&self, addr: usize) -> bool {
        let frame = addr / FRAME_SIZE;
        self.get_bit(frame) && self.used_frames.load(Ordering::SeqCst) < frame
    }

    pub fn reset_all_bits(&self) {
        let words = (self.total_frames + 63) / 64;
        unsafe {
            for i in 0..words {
                self.bitmap.add(i).write(0);
            }
        }
        self.used_frames.store(0, Ordering::SeqCst);
        self.reserved_frames.store(0, Ordering::SeqCst);
    }

    pub fn alloc_kernel_stack(&self) -> Option<usize> {
        self.alloc_frames_contiguous(2)
    }

    pub fn free_kernel_stack(&self, addr: usize) {
        self.free_frames_contiguous(addr, 2);
    }

    pub fn is_bitmap_full(&self) -> bool {
        for i in 0..((self.total_frames + 63) / 64) {
            unsafe {
                if self.bitmap.add(i).read() != u64::MAX {
                    return false;
                }
            }
        }
        true
    }

    pub fn alloc_user_page(&self) -> Option<usize> {
        self.alloc_frame()
    }

    pub fn free_user_page(&self, addr: usize) {
        self.free_frame(addr);
    }

    pub fn get_bitmap_word(&self, index: usize) -> u64 {
        if index < (self.total_frames + 63) / 64 {
            unsafe { self.bitmap.add(index).read() }
        } else {
            0
        }
    }

    pub fn set_used_frames(&self, count: usize) {
        self.used_frames.store(count, Ordering::SeqCst);
    }

    pub fn increment_used_frames(&self, count: usize) {
        self.used_frames.fetch_add(count, Ordering::SeqCst);
    }

    pub fn decrement_used_frames(&self, count: usize) {
        self.used_frames.fetch_sub(count, Ordering::SeqCst);
    }

    pub fn update_reservation(&self, delta: isize) {
        if delta > 0 {
            self.reserved_frames.fetch_add(delta as usize, Ordering::SeqCst);
        } else {
            self.reserved_frames.fetch_sub((-delta) as usize, Ordering::SeqCst);
        }
    }

    pub fn get_bitmap_size(&self) -> usize {
        ((self.total_frames + 63) / 64) * 8
    }

    pub fn validate_and_alloc(&self, addr: usize, size: usize) -> bool {
        let start_frame = addr / FRAME_SIZE;
        let end_frame = (addr + size + FRAME_SIZE - 1) / FRAME_SIZE;
        for i in start_frame..end_frame {
            if i >= self.total_frames || self.get_bit(i) {
                return false;
            }
        }
        for i in start_frame..end_frame {
            self.set_bit(i);
        }
        self.used_frames.fetch_add(end_frame - start_frame, Ordering::SeqCst);
        true
    }

    pub fn alloc_page_table(&self) -> Option<usize> {
        self.alloc_frame()
    }

    pub fn free_page_table(&self, addr: usize) {
        self.free_frame(addr);
    }

    pub fn alloc_identity_map(&self, size: usize) -> Option<usize> {
        self.alloc_frames_contiguous((size + FRAME_SIZE - 1) / FRAME_SIZE)
    }

    pub fn get_fragmentation_info(&self) -> (usize, usize) {
        let mut free_blocks = 0;
        let mut is_in_free_block = false;
        
        for i in 0..self.total_frames {
            if !self.get_bit(i) {
                if !is_in_free_block {
                    free_blocks += 1;
                    is_in_free_block = true;
                }
            } else {
                is_in_free_block = false;
            }
        }
        (free_blocks, self.get_free_frames())
    }
}

