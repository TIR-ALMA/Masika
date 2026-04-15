use core::arch::asm;

#[repr(C, packed)]
pub struct Descriptor {
    limit: u16,
    base_lo: u16,
    base_mid: u8,
    access: u8,
    flags_limit_hi: u8,
    base_hi: u8,
}

#[repr(C, packed)]
pub struct TssEntry {
    reserved: u32,
    rsp0_lo: u32,
    rsp0_hi: u32,
    rsp1_lo: u32,
    rsp1_hi: u32,
    rsp2_lo: u32,
    rsp2_hi: u32,
    reserved2: u64,
    ist1_lo: u32,
    ist1_hi: u32,
    ist2_lo: u32,
    ist2_hi: u32,
    ist3_lo: u32,
    ist3_hi: u32,
    ist4_lo: u32,
    ist4_hi: u32,
    ist5_lo: u32,
    ist5_hi: u32,
    ist6_lo: u32,
    ist6_hi: u32,
    ist7_lo: u32,
    ist7_hi: u32,
    reserved3: u64,
    iopb_offset: u16,
}

static mut TSS: TssEntry = TssEntry {
    reserved: 0,
    rsp0_lo: 0,
    rsp0_hi: 0,
    rsp1_lo: 0,
    rsp1_hi: 0,
    rsp2_lo: 0,
    rsp2_hi: 0,
    reserved2: 0,
    ist1_lo: 0,
    ist1_hi: 0,
    ist2_lo: 0,
    ist2_hi: 0,
    ist3_lo: 0,
    ist3_hi: 0,
    ist4_lo: 0,
    ist4_hi: 0,
    ist5_lo: 0,
    ist5_hi: 0,
    ist6_lo: 0,
    ist6_hi: 0,
    ist7_lo: 0,
    ist7_hi: 0,
    reserved3: 0,
    iopb_offset: 0,
};

pub struct Gdt {
    entries: [Descriptor; 8],
}

impl Gdt {
    pub fn new() -> Self {
        Self { entries: [Descriptor { limit: 0, base_lo: 0, base_mid: 0, access: 0, flags_limit_hi: 0, base_hi: 0 }; 8] }
    }

    pub fn init(&mut self) {
        self.entries[1] = Self::make_code_seg(0, 0xfffff);
        self.entries[2] = Self::make_data_seg(0, 0xfffff);
        self.entries[3] = Self::make_tss_desc();
        
        let ptr = DescriptorTablePointer { limit: (core::mem::size_of_val(&self.entries) - 1) as u16, base: self.entries.as_ptr() };
        unsafe {
            asm!("lgdt [{}]", in(reg) &ptr, options(att_syntax));
            asm!("mov ax, 0x10", out("ax") _, options(att_syntax));
            asm!("mov ds, ax", "mov es, ax", "mov fs, ax", "mov gs, ax", "mov ss, ax", options(att_syntax));
            asm!("ljmp $0x8, $1f", "1:", options(att_syntax));
        }
    }

    fn make_code_seg(base: u32, limit: u32) -> Descriptor {
        Descriptor {
            limit: limit as u16,
            base_lo: base as u16,
            base_mid: (base >> 16) as u8,
            access: 0x9a,
            flags_limit_hi: ((limit >> 16) as u8 & 0x0f) | 0xa0,
            base_hi: (base >> 24) as u8,
        }
    }

    fn make_data_seg(base: u32, limit: u32) -> Descriptor {
        Descriptor {
            limit: limit as u16,
            base_lo: base as u16,
            base_mid: (base >> 16) as u8,
            access: 0x92,
            flags_limit_hi: ((limit >> 16) as u8 & 0x0f) | 0xc0,
            base_hi: (base >> 24) as u8,
        }
    }

    fn make_tss_desc(&self) -> Descriptor {
        let addr = unsafe { &TSS as *const _ as u64 };
        Descriptor {
            limit: (core::mem::size_of::<TssEntry>() - 1) as u16,
            base_lo: addr as u16,
            base_mid: (addr >> 16) as u8,
            access: 0x89,
            flags_limit_hi: ((addr >> 24) as u8 & 0x0f) | 0x40,
            base_hi: (addr >> 32) as u8,
        }
    }
}

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: *const Descriptor,
}

pub fn enable_sse() {
    unsafe {
        let mut cr0: usize;
        let mut cr4: usize;
        asm!("mov {}, cr0", out(reg) cr0, options(att_syntax));
        cr0 &= !0x04;
        cr0 |= 0x02;
        asm!("mov cr0, {}", in(reg) cr0, options(att_syntax));

        asm!("mov {}, cr4", out(reg) cr4, options(att_syntax));
        cr4 |= 0x600;
        asm!("mov cr4, {}", in(reg) cr4, options(att_syntax));
    }
}

pub fn enable_avx() {
    unsafe {
        let mut cr4: usize;
        asm!("mov {}, cr4", out(reg) cr4, options(att_syntax));
        cr4 |= 0x20000 | 0x40000;
        asm!("mov cr4, {}", in(reg) cr4, options(att_syntax));
    }
}

pub fn read_cr0() -> usize {
    let value: usize;
    unsafe { asm!("mov {}, cr0", out(reg) value, options(att_syntax)) };
    value
}

pub fn write_cr0(value: usize) {
    unsafe { asm!("mov cr0, {}", in(reg) value, options(att_syntax)) };
}

pub fn read_cr3() -> usize {
    let value: usize;
    unsafe { asm!("mov {}, cr3", out(reg) value, options(att_syntax)) };
    value
}

pub fn write_cr3(value: usize) {
    unsafe { asm!("mov cr3, {}", in(reg) value, options(att_syntax)) };
}

pub fn read_cr4() -> usize {
    let value: usize;
    unsafe { asm!("mov {}, cr4", out(reg) value, options(att_syntax)) };
    value
}

pub fn write_cr4(value: usize) {
    unsafe { asm!("mov cr4, {}", in(reg) value, options(att_syntax)) };
}

pub fn cli() {
    unsafe { asm!("cli", options(att_syntax)) };
}

pub fn sti() {
    unsafe { asm!("sti", options(att_syntax)) };
}

pub fn halt() {
    unsafe { asm!("hlt", options(att_syntax)) };
}

pub fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
            options(att_syntax)
        );
    }
    ((high as u64) << 32) | (low as u64)
}

pub fn write_msr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") low,
            in("edx") high,
            options(att_syntax)
        );
    }
}

pub fn cpuid(leaf: u32) -> (u32, u32, u32, u32) {
    let a: u32;
    let b: u32;
    let c: u32;
    let d: u32;
    unsafe {
        asm!(
            "cpuid",
            in("eax") leaf,
            out("eax") a,
            out("ebx") b,
            out("ecx") c,
            out("edx") d,
            options(att_syntax)
        );
    }
    (a, b, c, d)
}

pub fn set_kernel_stack(stack_top: u64) {
    unsafe {
        TSS.rsp0_lo = stack_top as u32;
        TSS.rsp0_hi = (stack_top >> 32) as u32;
    }
}

pub fn enable_fpu_exceptions() {
    unsafe {
        let mut mxcsr: u32;
        asm!("stmxcsr [{}]", in(reg) &mut mxcsr, options(att_syntax));
        mxcsr &= !(1 << 15); // Clear flush-to-zero
        mxcsr &= !(1 << 6);  // Clear denormals-are-zero
        asm!("ldmxcsr [{}]", in(reg) &mxcsr, options(att_syntax));
    }
}

pub fn read_eflags() -> usize {
    let r: usize;
    unsafe { asm!("pushfq; pop {}", out(reg) r, options(att_syntax)) };
    r
}

pub fn write_eflags(flags: usize) {
    unsafe { asm!("push {}; popfq", in(reg) flags, options(att_syntax)) };
}

pub fn invlpg(addr: usize) {
    unsafe { asm!("invlpg [{}]", in(reg) addr, options(att_syntax)) };
}

pub fn lgdt(gdt_ptr: *const DescriptorTablePointer) {
    unsafe { asm!("lgdt [{}]", in(reg) gdt_ptr, options(att_syntax)) };
}

pub fn lidt(idt_ptr: *const DescriptorTablePointer) {
    unsafe { asm!("lidt [{}]", in(reg) idt_ptr, options(att_syntax)) };
}

pub fn load_tr(sel: u16) {
    unsafe { asm!("ltr ax", in("ax") sel, options(att_syntax)) };
}

pub fn enable_nxe_bit() {
    write_msr(0xC0000080, read_msr(0xC0000080) | 1);
}

pub fn enable_write_protect() {
    let mut cr0 = read_cr0();
    cr0 |= 1 << 16;
    write_cr0(cr0);
}

pub fn disable_write_protect() {
    let mut cr0 = read_cr0();
    cr0 &= !(1 << 16);
    write_cr0(cr0);
}

pub fn enable_global_pages() {
    let mut cr4 = read_cr4();
    cr4 |= 1 << 7;
    write_cr4(cr4);
}

pub fn get_cpu_vendor_string() -> [u8; 12] {
    let (ebx, ecx, edx, _) = cpuid(0);
    let mut vendor = [0u8; 12];
    (vendor[0..4]).copy_from_slice(&ebx.to_le_bytes());
    (vendor[4..8]).copy_from_slice(&edx.to_le_bytes());
    (vendor[8..12]).copy_from_slice(&ecx.to_le_bytes());
    vendor
}

pub fn has_x2apic() -> bool {
    let (_, ebx, _, _) = cpuid(1);
    (ebx >> 21) & 1 == 1
}

pub fn has_fsgsbase() -> bool {
    let (_, ebx, _, _) = cpuid(7);
    (ebx >> 0) & 1 == 1
}

pub fn has_smap() -> bool {
    let (_, ebx, _, _) = cpuid(7);
    (ebx >> 20) & 1 == 1
}

pub fn has_smep() -> bool {
    let (_, ebx, _, _) = cpuid(7);
    (ebx >> 7) & 1 == 1
}

pub fn enable_smap() {
    if has_smap() {
        let mut cr4 = read_cr4();
        cr4 |= 1 << 21;
        write_cr4(cr4);
    }
}

pub fn clear_smap() {
    let mut cr4 = read_cr4();
    cr4 &= !(1 << 21);
    write_cr4(cr4);
}

pub fn enable_smep() {
    if has_smep() {
        let mut cr4 = read_cr4();
        cr4 |= 1 << 20;
        write_cr4(cr4);
    }
}

pub fn read_fs_base() -> u64 {
    read_msr(0xC0000100)
}

pub fn write_fs_base(base: u64) {
    write_msr(0xC0000100, base);
}

pub fn read_gs_base() -> u64 {
    read_msr(0xC0000101)
}

pub fn write_gs_base(base: u64) {
    write_msr(0xC0000101, base);
}

pub fn enable_fsgsbase_if_supported() {
    if has_fsgsbase() {
        let mut cr4 = read_cr4();
        cr4 |= 1 << 16;
        write_cr4(cr4);
    }
}

pub fn rdfsbase() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!("rdfsbase eax", out("eax") low, options(att_syntax));
        asm!("rdgsbase eax", out("eax") high, options(att_syntax));
    }
    ((high as u64) << 32) | (low as u64)
}

pub fn wrfsbase(val: u64) {
    let low = val as u32;
    let high = (val >> 32) as u32;
    unsafe {
        asm!("wrfsbase eax", in("eax") low, options(att_syntax));
        asm!("wrgsbase eax", in("eax") high, options(att_syntax));
    }
}

pub fn wrgsbase(val: u64) {
    let low = val as u32;
    let high = (val >> 32) as u32;
    unsafe {
        asm!("wrgsbase eax", in("eax") low, options(att_syntax));
        asm!("wrgsbase eax", in("eax") high, options(att_syntax));
    }
}

pub fn enable_pat() {
    let pat_msr = read_msr(0x277);
    write_msr(0x277, pat_msr);
}

pub fn get_pat() -> u64 {
    read_msr(0x277)
}

pub fn set_pat(pat: u64) {
    write_msr(0x277, pat);
}

pub fn enable_pge() {
    let mut cr4 = read_cr4();
    cr4 |= 1 << 7;
    write_cr4(cr4);
}

pub fn disable_pge() {
    let mut cr4 = read_cr4();
    cr4 &= !(1 << 7);
    write_cr4(cr4);
}

pub fn enable_pae() {
    let mut cr4 = read_cr4();
    cr4 |= 1 << 5;
    write_cr4(cr4);
}

pub fn disable_pae() {
    let mut cr4 = read_cr4();
    cr4 &= !(1 << 5);
    write_cr4(cr4);
}

pub fn enable_pcid() {
    let mut cr4 = read_cr4();
    cr4 |= 1 << 17;
    write_cr4(cr4);
}

pub fn disable_pcid() {
    let mut cr4 = read_cr4();
    cr4 &= !(1 << 17);
    write_cr4(cr4);
}

pub fn enable_pge_if_supported() {
    let (_, _, _, edx) = cpuid(1);
    if (edx >> 13) & 1 == 1 {
        enable_pge();
    }
}

pub fn enable_page_attribute_table() {
    let (_, _, _, edx) = cpuid(1);
    if (edx >> 16) & 1 == 1 {
        enable_pat();
    }
}

pub fn enable_supervisor_mode_execution_protection() {
    let mut efer = read_msr(0xC0000080);
    efer |= 1 << 11;
    write_msr(0xC0000080, efer);
}

pub fn disable_supervisor_mode_execution_protection() {
    let mut efer = read_msr(0xC0000080);
    efer &= !(1 << 11);
    write_msr(0xC0000080, efer);
}

pub fn enable_supervisor_mode_access_prevention() {
    let mut efer = read_msr(0xC0000080);
    efer |= 1 << 12;
    write_msr(0xC0000080, efer);
}

pub fn disable_supervisor_mode_access_prevention() {
    let mut efer = read_msr(0xC0000080);
    efer &= !(1 << 12);
    write_msr(0xC0000080, efer);
}

pub fn enable_fpu() {
    let mut cr0 = read_cr0();
    cr0 &= !(1 << 2);
    cr0 |= 1 << 1;
    write_cr0(cr0);
}

pub fn disable_fpu() {
    let mut cr0 = read_cr0();
    cr0 |= 1 << 2;
    cr0 &= !(1 << 1);
    write_cr0(cr0);
}

pub fn wait_for_interrupt() {
    unsafe { asm!("sti; hlt", options(att_syntax)) };
}

pub fn pause() {
    unsafe { asm!("pause", options(att_syntax)) };
}

pub fn get_cpu_frequency_khz() -> u64 {
    let (_, _, _, edx) = cpuid(0x15);
    if edx != 0 {
        let tsc_frequency = (edx as u64) * get_processor_base_frequency_khz();
        return tsc_frequency;
    }
    0
}

fn get_processor_base_frequency_khz() -> u64 {
    let (_, _, _, ebx) = cpuid(0x16);
    ((ebx & 0xFFFF) as u64) * 1000
}

pub fn enable_monitor_mwait() {
    let (_, edx, _, _) = cpuid(5);
    if (edx & 1) == 1 {
        let mut cr4 = read_cr4();
        cr4 |= 1 << 4;
        write_cr4(cr4);
    }
}

pub fn monitor(eax: u32, ecx: u32, edx: u32) {
    unsafe {
        asm!(
            "monitor",
            in("eax") eax,
            in("ecx") ecx,
            in("edx") edx,
            options(att_syntax)
        );
    }
}

pub fn mwait(eax: u32, ecx: u32) {
    unsafe {
        asm!(
            "mwait",
            in("eax") eax,
            in("ecx") ecx,
            options(att_syntax)
        );
    }
}

pub fn enable_adaptive_tickless() {
    enable_monitor_mwait();
}

pub fn read_time_stamp_counter() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!(
            "rdtsc",
            out("eax") low,
            out("edx") high,
            options(att_syntax)
        );
    }
    ((high as u64) << 32) | (low as u64)
}

pub fn read_time_stamp_counter_high() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!(
            "rdtscp",
            out("eax") low,
            out("edx") high,
            out("ecx") _,
            options(att_syntax)
        );
    }
    ((high as u64) << 32) | (low as u64)
}

pub fn enable_rdtscp() {
    let (_, _, ecx, _) = cpuid(0x80000001);
    if (ecx >> 27) & 1 == 1 {
        let mut efer = read_msr(0xC0000080);
        efer |= 1 << 15;
        write_msr(0xC0000080, efer);
    }
}

pub fn disable_rdtscp() {
    let mut efer = read_msr(0xC0000080);
    efer &= !(1 << 15);
    write_msr(0xC0000080, efer);
}

pub fn enable_syscall_msrs() {
    let (_, _, ecx, _) = cpuid(0x80000001);
    if (ecx >> 11) & 1 == 1 {
        let mut efer = read_msr(0xC0000080);
        efer |= 1;
        write_msr(0xC0000080, efer);
    }
}

pub fn set_star_msr(kernel_cs: u64, user_cs: u64) {
    write_msr(0xC0000081, (kernel_cs << 32) | user_cs);
}

pub fn set_lstar_msr(handler: u64) {
    write_msr(0xC0000082, handler);
}

pub fn set_sfmask_msr(mask: u64) {
    write_msr(0xC0000084, mask);
}

pub fn enable_xsave() {
    let (_, _, ecx, _) = cpuid(1);
    if (ecx >> 26) & 1 == 1 {
        let mut cr4 = read_cr4();
        cr4 |= 1 << 18;
        write_cr4(cr4);
    }
}

pub fn enable_fxrstor_optimizations() {
    let (_, _, ecx, _) = cpuid(1);
    if (ecx >> 24) & 1 == 1 {
        let mut cr4 = read_cr4();
        cr4 |= 1 << 9;
        write_cr4(cr4);
    }
}

pub fn enable_xsaveopt() {
    let (_, _, ecx, _) = cpuid(0xD);
    if (ecx >> 0) & 1 == 1 {
        let mut xcr0 = read_msr(0x00000400);
        xcr0 |= 1;
        write_msr(0x00000400, xcr0);
    }
}

pub fn read_xcr0() -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!(
            "xgetbv",
            in("ecx") 0,
            out("eax") low,
            out("edx") high,
            options(att_syntax)
        );
    }
    ((high as u64) << 32) | (low as u64)
}

pub fn write_xcr0(value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    unsafe {
        asm!(
            "xsetbv",
            in("ecx") 0,
            in("eax") low,
            in("edx") high,
            options(att_syntax)
        );
    }
}

pub fn get_xsave_feature_mask(index: u32) -> u64 {
    if index == 0 {
        return read_xcr0();
    }
    read_msr(0x00000400 + index as u64)
}

pub fn xsave_area_size() -> u32 {
    let (_, ebx, _, _) = cpuid(0xD);
    ebx
}

pub fn xsave_compacted_enabled() -> bool {
    let (_, _, ecx, _) = cpuid(0xD);
    (ecx >> 1) & 1 == 1
}

pub fn enable_cet_shadow_stack() {
    let (_, _, ecx, _) = cpuid(7);
    if (ecx >> 20) & 1 == 1 {
        let mut efer = read_msr(0xC0000080);
        efer |= 1 << 16;
        write_msr(0xC0000080, efer);
    }
}

pub fn enable_cet_indirect_branch_tracking() {
    let (_, _, ecx, _) = cpuid(7);
    if (ecx >> 20) & 1 == 1 {
        let mut efer = read_msr(0xC0000080);
        efer |= 1 << 17;
        write_msr(0xC0000080, efer);
    }
}

pub fn enable_umip() {
    let (_, _, ecx, _) = cpuid(7);
    if (ecx >> 2) & 1 == 1 {
        let mut cr4 = read_cr4();
        cr4 |= 1 << 11;
        write_cr4(cr4);
    }
}

pub fn disable_umip() {
    let mut cr4 = read_cr4();
    cr4 &= !(1 << 11);
    write_cr4(cr4);
}

pub fn enable_pku() {
    let (_, _, ecx, _) = cpuid(7);
    if (ecx >> 3) & 1 == 1 {
        let mut cr4 = read_cr4();
        cr4 |= 1 << 22;
        write_cr4(cr4);
    }
}

pub fn disable_pku() {
    let mut cr4 = read_cr4();
    cr4 &= !(1 << 22);
    write_cr4(cr4);
}

pub fn read_pkru() -> u32 {
    let value: u32;
    unsafe { asm!("rdpkru", out("eax") value, options(att_syntax)) };
    value
}

pub fn write_pkru(value: u32) {
    unsafe { asm!("wrpkru", in("eax") value, options(att_syntax)) };
}

pub fn enable_rdpid() {
    let (_, _, ecx, _) = cpuid(7);
    if (ecx >> 22) & 1 == 1 {
        let mut cr4 = read_cr4();
        cr4 |= 1 << 23;
        write_cr4(cr4);
    }
}

pub fn enable_tme() {
    let (_, _, ecx, _) = cpuid(7);
    if (ecx >> 13) & 1 == 1 {
        let mut ia32_tme_activation = read_msr(0x982);
        ia32_tme_activation |= 1;
        write_msr(0x982, ia32_tme_activation);
    }
}

pub fn enable_keylocker() {
    let (_, _, ecx, _) = cpuid(7);
    if (ecx >> 23) & 1 == 1 {
        let mut cr4 = read_cr4();
        cr4 |= 1 << 24;
        write_cr4(cr4);
    }
}

pub fn enable_wbnoinvd() {
    let (_, _, ecx, _) = cpuid(7);
    if (ecx >> 9) & 1 == 1 {
        let mut ia32_misc_enable = read_msr(0x1a0);
        ia32_misc_enable |= 1 << 12;
        write_msr(0x1a0, ia32_misc_enable);
    }
}

pub fn wbnoinvd() {
    unsafe { asm!("wbnoinvd", options(att_syntax)) };
}

pub fn wbpinvd() {
    unsafe { asm!("wbpinvd", options(att_syntax)) };
}

pub fn enable_pconfig() {
    let (_, _, ecx, _) = cpuid(7);
    if (ecx >> 18) & 1 == 1 {
        let mut ia32_misc_enable = read_msr(0x1a0);
        ia32_misc_enable |= 1 << 18;
        write_msr(0x1a0, ia32_misc_enable);
    }
}

pub fn pconfig() {
    unsafe { asm!("pconfig", options(att_syntax)) };
}

pub fn enable_ibt() {
    let (_, _, edx, _) = cpuid(7);
    if (edx >> 20) & 1 == 1 {
        let mut ia32_efer = read_msr(0xC0000080);
        ia32_efer |= 1 << 14;
        write_msr(0xC0000080, ia32_efer);
    }
}

pub fn enable_ssbd() {
    let (_, _, edx, _) = cpuid(7);
    if (edx >> 31) & 1 == 1 {
        let mut ia32_misc_enable = read_msr(0x1a0);
        ia32_misc_enable |= 1 << 24;
        write_msr(0x1a0, ia32_misc_enable);
    }
}

pub fn enable_stibp() {
    let (_, _, edx, _) = cpuid(7);
    if (edx >> 27) & 1 == 1 {
        let mut ia32_misc_enable = read_msr(0x1a0);
        ia32_misc_enable |= 1 << 15;
        write_msr(0x1a0, ia32_misc_enable);
    }
}

pub fn enable_ibpb() {
    let (_, _, edx, _) = cpuid(7);
    if (edx >> 26) & 1 == 1 {
        let mut ia32_misc_enable = read_msr(0x1a0);
        ia32_misc_enable |= 1 << 14;
        write_msr(0x1a0, ia32_misc_enable);
    }
}

pub fn ibpb() {
    let mut spec_ctrl = read_msr(0x48);
    spec_ctrl |= 1;
    write_msr(0x48, spec_ctrl);
}

pub fn stibp() {
    let mut spec_ctrl = read_msr(0x48);
    spec_ctrl |= 1 << 1;
    write_msr(0x48, spec_ctrl);
}

pub fn ssbd() {
    let mut spec_ctrl = read_msr(0x48);
    spec_ctrl |= 1 << 2;
    write_msr(0x48, spec_ctrl);
}

pub fn clear_spec_ctrl() {
    write_msr(0x48, 0);
}

pub fn enable_arch_capabilities() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 29) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_flush_l1d() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 28) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 1;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn flush_l1d_cache() {
    unsafe { asm!("flush_l1d", options(att_syntax)) };
}

pub fn enable_md_clear() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 27) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 2;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn md_clear() {
    unsafe { asm!("md_clear", options(att_syntax)) };
}

pub fn enable_tsx_ctrl() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 26) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 3;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn disable_taa() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 25) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 4;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_bnti() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 24) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 5;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_rtm_always_abort() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 23) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 6;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ppin() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 22) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 7;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_cnp() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 21) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 8;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_lbrv() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 20) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 9;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ptw() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 19) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 10;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_pconfig_lock() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 18) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 11;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_tme_key Locker() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 17) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 12;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_hwp() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 16) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 13;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_tme_multi_key() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 15) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 14;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ras_admin() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 14) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 15;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ras_cap() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 13) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 16;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ras_int() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 12) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 17;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ras_log() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 11) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 18;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ras_report() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 10) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 19;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ras_status() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 9) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 20;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ras_trigger() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 8) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 21;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ras_config() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 7) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 22;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ras_diag() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 6) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 23;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ras_error() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 5) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 24;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ras_fault() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 4) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 25;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ras_info() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 3) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 26;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ras_state() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 2) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 27;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ras_test() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 1) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 28;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_ras_debug() {
    let (_, _, _, edx) = cpuid(7);
    if (edx >> 0) & 1 == 1 {
        let mut ia32_arch_caps = read_msr(0x10a);
        ia32_arch_caps |= 1 << 29;
        write_msr(0x10a, ia32_arch_caps);
    }
}

pub fn enable_all_features() {
    enable_sse();
    enable_avx();
    enable_nxe_bit();
    enable_write_protect();
    enable_global_pages();
    enable_smap();
    enable_smep();
    enable_fsgsbase_if_supported();
    enable_pat();
    enable_pge();
    enable_pae();
    enable_pcid();
    enable_supervisor_mode_execution_protection();
    enable_supervisor_mode_access_prevention();
    enable_fpu();
    enable_monitor_mwait();
    enable_adaptive_tickless();
    enable_rdtscp();
    enable_syscall_msrs();
    enable_xsave();
    enable_fxrstor_optimizations();
    enable_xsaveopt();
    enable_cet_shadow_stack();
    enable_cet_indirect_branch_tracking();
    enable_umip();
    enable_pku();
    enable_rdpid();
    enable_tme();
    enable_keylocker();
    enable_wbnoinvd();
    enable_pconfig();
    enable_ibt();
    enable_ssbd();
    enable_stibp();
    enable_ibpb();
    enable_arch_capabilities();
    enable_flush_l1d();
    enable_md_clear();
    enable_tsx_ctrl();
    disable_taa();
    enable_bnti();
    enable_rtm_always_abort();
    enable_ppin();
    enable_cnp();
    enable_lbrv();
    enable_ptw();
    enable_pconfig_lock();
    enable_tme_key_locker();
    enable_hwp();
    enable_tme_multi_key();
    enable_ras_admin();
    enable_ras_cap();
    enable_ras_int();
    enable_ras_log();
    enable_ras_report();
    enable_ras_status();
    enable_ras_trigger();
    enable_ras_config();
    enable_ras_diag();
    enable_ras_error();
    enable_ras_fault();
    enable_ras_info();
    enable_ras_state();
    enable_ras_test();
    enable_ras_debug();
}

