use core::ptr;
use core::arch::x86_64::_mm_mfence;
use core::sync::atomic::{AtomicU32, Ordering};

const LAPIC_BASE_MSR: u32 = 0x1B;
const IOAPIC_BASE: usize = 0xFEC00000;
const DEFAULT_IOAPIC_GSI_BASE: u32 = 0;

static mut LOCAL_APIC_ADDR: *mut u32 = ptr::null_mut();
static mut IOAPIC_ADDR: *mut u32 = ptr::null_mut();
static mut IOAPIC_COUNT: u32 = 0;
static mut IOAPIC_INFO: [IoApicInfo; 8] = [IoApicInfo::new(); 8];

#[repr(C)]
struct IoApicInfo {
    addr: usize,
    gsi_base: u32,
    max_redir: u8,
}

impl IoApicInfo {
    const fn new() -> Self {
        Self {
            addr: 0,
            gsi_base: 0,
            max_redir: 0,
        }
    }
}

pub struct Apic {
    lapic_id: u8,
    version: u8,
    max_lvt: u8,
}

impl Apic {
    pub fn new() -> Self {
        unsafe {
            let msr_val = x86::msr::rdmsr(LAPIC_BASE_MSR);
            LOCAL_APIC_ADDR = (msr_val as *mut u32).add(0x200 >> 2);
            IOAPIC_ADDR = IOAPIC_BASE as *mut u32;
        }
        let lapic_id = unsafe { read_lapic_reg(0x20) } >> 24;
        let version = (unsafe { read_lapic_reg(0x30) } & 0xFF) as u8;
        let max_lvt = ((unsafe { read_lapic_reg(0x30) } >> 16) & 0xFF) as u8;
        
        let apic = Self { lapic_id, version, max_lvt };
        apic.init_local();
        apic.init_ioapics();
        apic.setup_timer();
        apic
    }

    fn init_local(&self) {
        unsafe {
            write_lapic_reg(0xF0, read_lapic_reg(0xF0) | 0x100);
            write_lapic_reg(0x80, 0x1B << 24);
            write_lapic_reg(0xD0, self.lapic_id as u32);
            write_lapic_reg(0xE0, 0);
            write_lapic_reg(0x320, 0x1FF);
            write_lapic_reg(0x350, 0);
            write_lapic_reg(0x360, 0);
            write_lapic_reg(0x370, 0);
            write_lapic_reg(0x380, 0);
            write_lapic_reg(0x390, 0);
            write_lapic_reg(0x3A0, 0);
            write_lapic_reg(0x3B0, 0);
            write_lapic_reg(0x3C0, 0);
            write_lapic_reg(0x3D0, 0);
        }
    }

    fn init_ioapics(&self) {
        unsafe {
            IOAPIC_INFO[0].addr = IOAPIC_BASE;
            IOAPIC_INFO[0].gsi_base = DEFAULT_IOAPIC_GSI_BASE;
            let ver = read_ioapic_reg_at(0, 0x01);
            IOAPIC_INFO[0].max_redir = ((ver >> 16) & 0xFF) as u8;
            IOAPIC_COUNT = 1;
            
            for i in 0..=IOAPIC_INFO[0].max_redir {
                self.mask_irq(i as u8);
            }
        }
    }

    pub fn setup_irq(&self, irq: u8, vector: u8) {
        let gsi = irq as u32;
        let ioapic_idx = 0;
        let idx = find_ioapic_by_gsi(gsi, ioapic_idx);
        if idx >= IOAPIC_COUNT as usize { return; }
        
        let reg_base = 0x10 + (irq as u32) * 2;
        let low = (vector as u32) & 0xFF;
        let high = ((self.lapic_id as u32) << 24) & 0xFF000000;
        unsafe {
            write_ioapic_reg_at(idx, reg_base, low);
            write_ioapic_reg_at(idx, reg_base + 1, high);
        }
    }

    pub fn mask_irq(&self, irq: u8) {
        let gsi = irq as u32;
        let ioapic_idx = 0;
        let idx = find_ioapic_by_gsi(gsi, ioapic_idx);
        if idx >= IOAPIC_COUNT as usize { return; }
        
        let reg_base = 0x10 + (irq as u32) * 2;
        unsafe {
            let val = read_ioapic_reg_at(idx, reg_base);
            write_ioapic_reg_at(idx, reg_base, val | (1 << 16));
        }
    }

    pub fn unmask_irq(&self, irq: u8) {
        let gsi = irq as u32;
        let ioapic_idx = 0;
        let idx = find_ioapic_by_gsi(gsi, ioapic_idx);
        if idx >= IOAPIC_COUNT as usize { return; }
        
        let reg_base = 0x10 + (irq as u32) * 2;
        unsafe {
            let val = read_ioapic_reg_at(idx, reg_base);
            write_ioapic_reg_at(idx, reg_base, val & !(1 << 16));
        }
    }

    pub fn send_eoi(&self) {
        unsafe { write_lapic_reg(0xB0, 0); }
    }

    fn setup_timer(&self) {
        unsafe {
            write_lapic_reg(0x3E0, 0x3);
            write_lapic_reg(0x320, 0x20 | (1 << 16));
        }
    }

    pub fn enable_spurious_vector(&self) {
        unsafe {
            write_lapic_reg(0xF0, (read_lapic_reg(0xF0) & !0xFF) | 0xFF | 0x100);
        }
    }

    pub fn send_ipi(&self, dest: u8, vector: u8) {
        let high = (dest as u32) << 24;
        let low = 0x000C4000 | (vector as u32);
        unsafe {
            write_lapic_reg(0x310, high);
            write_lapic_reg(0x300, low);
        }
    }

    pub fn send_ipi_init(&self, dest: u8) {
        let high = (dest as u32) << 24;
        let low = 0x000C4500;
        unsafe {
            write_lapic_reg(0x310, high);
            write_lapic_reg(0x300, low);
        }
    }

    pub fn send_ipi_startup(&self, dest: u8, vector: u8) {
        let high = (dest as u32) << 24;
        let low = 0x000C4600 | ((vector as u32) >> 12);
        unsafe {
            write_lapic_reg(0x310, high);
            write_lapic_reg(0x300, low);
        }
    }

    pub fn get_lapic_id(&self) -> u8 {
        self.lapic_id
    }

    pub fn calibrate_timer(&self, loops: u32) {
        unsafe {
            write_lapic_reg(0x3E0, loops);
            write_lapic_reg(0x320, 0x20 | (1 << 16));
            while read_lapic_reg(0x390) != 0 {}
        }
    }

    pub fn set_timer_periodic(&self, vector: u8, divisor: u8, count: u32) {
        unsafe {
            write_lapic_reg(0x3E0, count);
            write_lapic_reg(0x380, (divisor_to_bits(divisor) as u32) << 24);
            write_lapic_reg(0x320, (vector as u32) | (0x2 << 17));
        }
    }

    pub fn set_timer_oneshot(&self, vector: u8, divisor: u8, count: u32) {
        unsafe {
            write_lapic_reg(0x3E0, count);
            write_lapic_reg(0x380, (divisor_to_bits(divisor) as u32) << 24);
            write_lapic_reg(0x320, (vector as u32) | (0x1 << 16));
        }
    }

    pub fn stop_timer(&self) {
        unsafe {
            write_lapic_reg(0x320, read_lapic_reg(0x320) | (1 << 16));
        }
    }

    pub fn enable_local_interrupts(&self) {
        unsafe {
            write_lapic_reg(0x350, read_lapic_reg(0x350) & !(1 << 16));
            write_lapic_reg(0x360, read_lapic_reg(0x360) & !(1 << 16));
        }
    }

    pub fn disable_local_interrupts(&self) {
        unsafe {
            write_lapic_reg(0x350, read_lapic_reg(0x350) | (1 << 16));
            write_lapic_reg(0x360, read_lapic_reg(0x360) | (1 << 16));
        }
    }

    pub fn read_isr_bit(&self, vector: u8) -> bool {
        let reg = 0x100 + ((vector as u32) / 32) * 0x10;
        let bit = (vector % 32) as u32;
        unsafe { (read_lapic_reg(reg) & (1 << bit)) != 0 }
    }

    pub fn read_irr_bit(&self, vector: u8) -> bool {
        let reg = 0x200 + ((vector as u32) / 32) * 0x10;
        let bit = (vector % 32) as u32;
        unsafe { (read_lapic_reg(reg) & (1 << bit)) != 0 }
    }

    pub fn wait_for_delivery(&self) {
        while unsafe { read_lapic_reg(0x300) } & (1 << 12) != 0 {}
    }

    pub fn detect_apic_ids(&self) -> [u8; 256] {
        let mut ids = [0u8; 256];
        for i in 0..256 {
            ids[i] = i as u8;
        }
        ids
    }

    pub fn configure_legacy_irqs(&self) {
        for i in 0..16 {
            self.setup_irq(i, 0x20 + i);
        }
    }

    pub fn route_irq_to_cpu(&self, irq: u8, cpu_id: u8) {
        let reg_base = 0x10 + (irq as u32) * 2;
        let low = (0x20 + irq as u32) & 0xFF;
        let high = ((cpu_id as u32) << 24) & 0xFF000000;
        unsafe {
            write_ioapic_reg_at(0, reg_base, low);
            write_ioapic_reg_at(0, reg_base + 1, high);
        }
    }

    pub fn get_max_lvt(&self) -> u8 {
        self.max_lvt
    }

    pub fn read_version(&self) -> u8 {
        self.version
    }

    pub fn send_nmi(&self) {
        unsafe {
            write_lapic_reg(0x300, 0x400 | (4 << 8));
        }
    }

    pub fn send_sipi(&self, dest: u8, vector: u8) {
        let high = (dest as u32) << 24;
        let low = 0x000C4600 | (vector as u32);
        unsafe {
            write_lapic_reg(0x310, high);
            write_lapic_reg(0x300, low);
        }
    }

    pub fn broadcast_ipi(&self, vector: u8) {
        let low = 0x00084000 | (vector as u32);
        unsafe {
            write_lapic_reg(0x300, low);
        }
    }

    pub fn broadcast_ipi_with_self(&self, vector: u8) {
        let low = 0x000C4000 | (vector as u32);
        unsafe {
            write_lapic_reg(0x300, low);
        }
    }

    pub fn read_icr(&self) -> u64 {
        let low = unsafe { read_lapic_reg(0x300) };
        let high = unsafe { read_lapic_reg(0x310) } & 0xFF000000;
        ((high as u64) << 32) | (low as u64)
    }

    pub fn configure_irq_trigger_mode(&self, irq: u8, level_triggered: bool) {
        let reg_base = 0x10 + (irq as u32) * 2;
        unsafe {
            let mut val = read_ioapic_reg_at(0, reg_base);
            if level_triggered {
                val |= 1 << 15;
            } else {
                val &= !(1 << 15);
            }
            write_ioapic_reg_at(0, reg_base, val);
        }
    }

    pub fn configure_irq_polarity(&self, irq: u8, active_low: bool) {
        let reg_base = 0x10 + (irq as u32) * 2;
        unsafe {
            let mut val = read_ioapic_reg_at(0, reg_base);
            if active_low {
                val |= 1 << 13;
            } else {
                val &= !(1 << 13);
            }
            write_ioapic_reg_at(0, reg_base, val);
        }
    }

    pub fn configure_irq_delivery_mode(&self, irq: u8, mode: u8) {
        let reg_base = 0x10 + (irq as u32) * 2;
        unsafe {
            let mut val = read_ioapic_reg_at(0, reg_base);
            val &= !(0x7 << 8);
            val |= ((mode as u32) & 0x7) << 8;
            write_ioapic_reg_at(0, reg_base, val);
        }
    }

    pub fn read_irq_entry(&self, irq: u8) -> (u32, u32) {
        let reg_base = 0x10 + (irq as u32) * 2;
        unsafe {
            let lo = read_ioapic_reg_at(0, reg_base);
            let hi = read_ioapic_reg_at(0, reg_base + 1);
            (lo, hi)
        }
    }

    pub fn set_irq_mask(&self, irq: u8, masked: bool) {
        let reg_base = 0x10 + (irq as u32) * 2;
        unsafe {
            let mut val = read_ioapic_reg_at(0, reg_base);
            if masked {
                val |= 1 << 16;
            } else {
                val &= !(1 << 16);
            }
            write_ioapic_reg_at(0, reg_base, val);
        }
    }

    pub fn get_irq_mask(&self, irq: u8) -> bool {
        let reg_base = 0x10 + (irq as u32) * 2;
        unsafe {
            let val = read_ioapic_reg_at(0, reg_base);
            (val & (1 << 16)) != 0
        }
    }

    pub fn get_ioapic_count(&self) -> u32 {
        unsafe { IOAPIC_COUNT }
    }

    pub fn get_ioapic_info(&self, idx: usize) -> Option<&IoApicInfo> {
        if idx < 8 && idx < unsafe { IOAPIC_COUNT as usize } {
            Some(unsafe { &IOAPIC_INFO[idx] })
        } else {
            None
        }
    }

    pub fn configure_irq_destination_mode(&self, irq: u8, logical: bool) {
        let reg_base = 0x10 + (irq as u32) * 2;
        unsafe {
            let mut val = read_ioapic_reg_at(0, reg_base);
            if logical {
                val |= 1 << 11;
            } else {
                val &= !(1 << 11);
            }
            write_ioapic_reg_at(0, reg_base, val);
        }
    }

    pub fn set_irq_destination_shorthand(&self, irq: u8, shorthand: u8) {
        let reg_base = 0x10 + (irq as u32) * 2;
        unsafe {
            let mut val = read_ioapic_reg_at(0, reg_base);
            val &= !(0x3 << 18);
            val |= ((shorthand as u32) & 0x3) << 18;
            write_ioapic_reg_at(0, reg_base, val);
        }
    }

    pub fn set_irq_remote_irr(&self, irq: u8, irr: bool) {
        let reg_base = 0x10 + (irq as u32) * 2;
        unsafe {
            let mut val = read_ioapic_reg_at(0, reg_base);
            if irr {
                val |= 1 << 14;
            } else {
                val &= !(1 << 14);
            }
            write_ioapic_reg_at(0, reg_base, val);
        }
    }

    pub fn check_ioapic_present(&self) -> bool {
        unsafe {
            let id = read_ioapic_reg_at(0, 0x00) >> 24;
            id != 0xFF
        }
    }

    pub fn print_apic_info(&self) {
        // For debugging only - not implemented here
    }
}

fn find_ioapic_by_gsi(gsi: u32, default_idx: usize) -> usize {
    for i in 0..unsafe { IOAPIC_COUNT as usize } {
        if gsi >= unsafe { IOAPIC_INFO[i].gsi_base } {
            let end = unsafe { IOAPIC_INFO[i].gsi_base + (IOAPIC_INFO[i].max_redir as u32) };
            if gsi <= end {
                return i;
            }
        }
    }
    default_idx
}

unsafe fn read_lapic_reg(offset: u32) -> u32 {
    _mm_mfence();
    ptr::read_volatile(LOCAL_APIC_ADDR.add((offset >> 2) as usize))
}

unsafe fn write_lapic_reg(offset: u32, value: u32) {
    _mm_mfence();
    ptr::write_volatile(LOCAL_APIC_ADDR.add((offset >> 2) as usize), value);
    _mm_mfence();
}

unsafe fn read_ioapic_reg_at(ioapic_idx: usize, offset: u32) -> u32 {
    let addr = IOAPIC_INFO[ioapic_idx].addr as *mut u32;
    ptr::write_volatile(addr, offset);
    ptr::read_volatile(addr.add(4))
}

unsafe fn write_ioapic_reg_at(ioapic_idx: usize, offset: u32, value: u32) {
    let addr = IOAPIC_INFO[ioapic_idx].addr as *mut u32;
    ptr::write_volatile(addr, offset);
    ptr::write_volatile(addr.add(4), value);
}

fn divisor_to_bits(divisor: u8) -> u8 {
    match divisor {
        1 => 0b000,
        2 => 0b001,
        4 => 0b010,
        8 => 0b011,
        16 => 0b100,
        32 => 0b101,
        64 => 0b110,
        128 => 0b111,
        _ => 0b011,
    }
