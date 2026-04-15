use core::arch::asm;
use core::ptr;

#[repr(C, packed)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    types_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const fn new() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            ist: 0,
            types_attr: 0,
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    fn set_handler(&mut self, addr: u64, attr: u8, selector: u16, ist: u8) {
        self.offset_low = (addr & 0xFFFF) as u16;
        self.offset_mid = ((addr >> 16) & 0xFFFF) as u16;
        self.offset_high = ((addr >> 32) & 0xFFFFFFFF) as u32;
        self.types_attr = attr;
        self.selector = selector;
        self.ist = ist & 0x7;
        self.reserved = 0;
    }
}

#[repr(C, packed)]
struct IdtDescriptor {
    size: u16,
    offset: u64,
}

static mut IDT: [IdtEntry; 256] = [IdtEntry::new(); 256];

extern "C" {
    fn divide_error_entry();
    fn debug_entry();
    fn nmi_entry();
    fn breakpoint_entry();
    fn overflow_entry();
    fn bound_range_exceeded_entry();
    fn invalid_opcode_entry();
    fn device_not_available_entry();
    fn double_fault_entry();
    fn coprocessor_segment_overrun_entry();
    fn invalid_tss_entry();
    fn segment_not_present_entry();
    fn stack_segment_fault_entry();
    fn general_protection_fault_entry();
    fn page_fault_entry();
    fn x87_fpu_floating_point_error_entry();
    fn alignment_check_entry();
    fn machine_check_entry();
    fn simd_floating_point_exception_entry();
    fn virtualization_exception_entry();
    fn security_exception_entry();
    fn syscall_entry();
    fn default_handler_entry();
}

pub fn init_idt() {
    unsafe {
        let entries = &mut IDT;
        entries[0].set_handler(divide_error_entry as u64, 0x8E, 0x08, 0);
        entries[1].set_handler(debug_entry as u64, 0x8E, 0x08, 0);
        entries[2].set_handler(nmi_entry as u64, 0x8E, 0x08, 0);
        entries[3].set_handler(breakpoint_entry as u64, 0x8E, 0x08, 0);
        entries[4].set_handler(overflow_entry as u64, 0x8E, 0x08, 0);
        entries[5].set_handler(bound_range_exceeded_entry as u64, 0x8E, 0x08, 0);
        entries[6].set_handler(invalid_opcode_entry as u64, 0x8E, 0x08, 0);
        entries[7].set_handler(device_not_available_entry as u64, 0x8E, 0x08, 0);
        entries[8].set_handler(double_fault_entry as u64, 0x8E, 0x08, 1); // IST
        entries[9].set_handler(coprocessor_segment_overrun_entry as u64, 0x8E, 0x08, 0);
        entries[10].set_handler(invalid_tss_entry as u64, 0x8E, 0x08, 0);
        entries[11].set_handler(segment_not_present_entry as u64, 0x8E, 0x08, 0);
        entries[12].set_handler(stack_segment_fault_entry as u64, 0x8E, 0x08, 0);
        entries[13].set_handler(general_protection_fault_entry as u64, 0x8E, 0x08, 0);
        entries[14].set_handler(page_fault_entry as u64, 0x8E, 0x08, 0);
        entries[15].set_handler(default_handler_entry as u64, 0x8E, 0x08, 0);
        entries[16].set_handler(x87_fpu_floating_point_error_entry as u64, 0x8E, 0x08, 0);
        entries[17].set_handler(alignment_check_entry as u64, 0x8E, 0x08, 0);
        entries[18].set_handler(machine_check_entry as u64, 0x8E, 0x08, 0);
        entries[19].set_handler(simd_floating_point_exception_entry as u64, 0x8E, 0x08, 0);
        entries[20].set_handler(virtualization_exception_entry as u64, 0x8E, 0x08, 0);
        entries[30].set_handler(security_exception_entry as u64, 0x8E, 0x08, 0);
        
        // IRQ handlers from 32 to 47
        for i in 32..48 {
            entries[i].set_handler(default_handler_entry as u64, 0x8E, 0x08, 0);
        }
        
        // Syscall entry at 0x80
        entries[0x80].set_handler(syscall_entry as u64, 0xEE, 0x08, 0); // User-callable

        for i in 48..256 {
            if !is_reserved(i) {
                entries[i].set_handler(default_handler_entry as u64, 0x8E, 0x08, 0);
            }
        }

        let descriptor = IdtDescriptor {
            size: (core::mem::size_of::<IdtEntry>() * 256 - 1) as u16,
            offset: IDT.as_ptr() as u64,
        };

        asm!(
            "lidt [{}]",
            in(reg) &descriptor,
            options(readonly, nostack)
        );
    }
}

fn is_reserved(vector: u8) -> bool {
    matches!(vector, 15 | 31)
}

#[no_mangle]
extern "C" fn handle_exception(
    vector: u64,
    err_code: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
) {
    match vector {
        0 => panic!("Divide by zero"),
        6 => panic!("Invalid opcode at {:#x}", rip),
        8 => panic!("Double fault with err_code={:#x} at {:#x}", err_code, rip),
        13 => panic!("General protection fault with err_code={:#x} at {:#x}", err_code, rip),
        14 => panic!("Page fault at {:#x} with err_code={:#x}", err_code, rip),
        _ => panic!("Unhandled exception {} at {:#x}", vector, rip),
    }
}

#[no_mangle]
extern "C" fn handle_irq(_vector: u64) {
    // Placeholder for IRQ handling
}

#[no_mangle]
extern "C" fn handle_syscall(number: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> u64 {
    // Placeholder for syscall dispatch
    0
}

pub fn load_idt() {
    unsafe {
        let descriptor = IdtDescriptor {
            size: (core::mem::size_of::<IdtEntry>() * 256 - 1) as u16,
            offset: IDT.as_ptr() as u64,
        };
        asm!(
            "lidt [{}]",
            in(reg) &descriptor,
            options(readonly, nostack)
        );
    }
}

pub fn enable_interrupts() {
    unsafe {
        asm!("sti", options(nomem, nostack));
    }
}

pub fn disable_interrupts() {
    unsafe {
        asm!("cli", options(nomem, nostack));
    }
}

pub fn get_idt_base() -> u64 {
    let base: u64;
    unsafe {
        asm!("sidt [{}]", in(reg) &base, options(readonly, nostack));
    }
    base
}

#[repr(u8)]
pub enum GateType {
    InterruptGate = 0xE,
    TrapGate = 0xF,
    TaskGate = 0x5,
}

pub struct IdtGate {
    pub offset: u64,
    pub selector: u16,
    pub gate_type: GateType,
    pub dpl: u8,
    pub present: bool,
}

impl IdtGate {
    pub fn new(offset: u64, selector: u16, gate_type: GateType, dpl: u8, present: bool) -> Self {
        Self {
            offset,
            selector,
            gate_type,
            dpl,
            present,
        }
    }

    pub fn build(&self) -> u8 {
        let mut attr = self.gate_type as u8;
        if self.present {
            attr |= 0x80;
        }
        attr |= (self.dpl & 0x3) << 5;
        attr
    }
}

pub fn set_gate(index: usize, handler: extern "C" fn(), gate_type: GateType, dpl: u8) {
    unsafe {
        if index < 256 {
            let attr = if gate_type == GateType::TrapGate {
                0xEF // Present + DPL + Trap Gate
            } else {
                0x8E // Present + DPL + Interrupt Gate
            };
            
            IDT[index].set_handler(handler as u64, attr, 0x08, 0);
        }
    }
}

pub fn install_isr(index: u8, handler: extern "C" fn()) {
    set_gate(index as usize, handler, GateType::InterruptGate, 0);
}

pub fn install_user_isr(index: u8, handler: extern "C" fn()) {
    set_gate(index as usize, handler, GateType::TrapGate, 3);
}

pub fn remap_pic() {
    unsafe {
        // Remap PIC to avoid conflicts with CPU exceptions
        asm!(
            "mov al, 0x11",
            "out 0x20, al",
            "out 0xA0, al",
            "mov al, 0x20",
            "out 0x21, al",
            "mov al, 0x28",
            "out 0xA1, al",
            "mov al, 0x04",
            "out 0x21, al",
            "mov al, 0x02",
            "out 0xA1, al",
            "mov al, 0x01",
            "out 0x21, al",
            "out 0xA1, al",
            "mov al, 0xFF",
            "out 0xA1, al",
            "mov al, 0xFB",
            "out 0x21, al",
            options(nostack)
        );
    }
}

pub fn mask_pic_irq(line: u8) {
    let port = if line < 8 { 0x21 } else { 0xA1 };
    let value = unsafe { core::ptr::read_volatile(port as *const u8) } | (1 << (line % 8));
    unsafe { core::ptr::write_volatile(port as *mut u8, value) };
}

pub fn unmask_pic_irq(line: u8) {
    let port = if line < 8 { 0x21 } else { 0xA1 };
    let value = unsafe { core::ptr::read_volatile(port as *const u8) } & !(1 << (line % 8));
    unsafe { core::ptr::write_volatile(port as *mut u8, value) };
}

#[derive(Debug)]
pub enum ExceptionVector {
    DivideError = 0,
    Debug = 1,
    NMI = 2,
    Breakpoint = 3,
    Overflow = 4,
    BoundRangeExceeded = 5,
    InvalidOpcode = 6,
    DeviceNotAvailable = 7,
    DoubleFault = 8,
    CoprocessorSegmentOverrun = 9,
    InvalidTSS = 10,
    SegmentNotPresent = 11,
    StackSegmentFault = 12,
    GeneralProtectionFault = 13,
    PageFault = 14,
    X87FPUFloatingPointError = 16,
    AlignmentCheck = 17,
    MachineCheck = 18,
    SIMDFloatingPointException = 19,
    VirtualizationException = 20,
    SecurityException = 30,
}

pub fn set_exception_handler(vector: ExceptionVector, handler: extern "C" fn()) {
    unsafe {
        let attr = 0x8E;
        IDT[vector as usize].set_handler(handler as u64, attr, 0x08, 0);
    }
}

pub fn enable_irq_line(irq: u8) {
    unsafe {
        if irq < 16 {
            let port = if irq < 8 { 0x21 } else { 0xA1 };
            let shift = if irq < 8 { irq } else { irq - 8 };
            let value = core::ptr::read_volatile(port as *const u8) & !(1 << shift);
            core::ptr::write_volatile(port as *mut u8, value);
        }
    }
}

pub fn disable_irq_line(irq: u8) {
    unsafe {
        if irq < 16 {
            let port = if irq < 8 { 0x21 } else { 0xA1 };
            let shift = if irq < 8 { irq } else { irq - 8 };
            let value = core::ptr::read_volatile(port as *const u8) | (1 << shift);
            core::ptr::write_volatile(port as *mut u8, value);
        }
    }
}

pub fn eoi(irq: u8) {
    unsafe {
        if irq >= 8 {
            core::ptr::write_volatile(0xA0u16 as *mut u8, 0x20);
        }
        core::ptr::write_volatile(0x20u16 as *mut u8, 0x20);
    }
}

pub fn set_ist_for_vector(vector: u8, ist_index: u8) {
    unsafe {
        if vector < 255 {
            IDT[vector as usize].ist = ist_index & 0x7;
        }
    }
}

pub fn get_ist_for_vector(vector: u8) -> u8 {
    unsafe {
        if vector < 255 {
            IDT[vector as usize].ist
        } else {
            0
        }
    }
}

pub fn install_irq_handler(irq: u8, handler: extern "C" fn()) {
    let vector = irq + 32;
    set_gate(vector as usize, handler, GateType::InterruptGate, 0);
}

pub fn install_user_irq_handler(irq: u8, handler: extern "C" fn()) {
    let vector = irq + 32;
    set_gate(vector as usize, handler, GateType::TrapGate, 3);
}

pub fn update_idt_entry(index: u8, offset: u64, selector: u16, ist: u8, types_attr: u8) {
    unsafe {
        IDT[index as usize].offset_low = (offset & 0xFFFF) as u16;
        IDT[index as usize].offset_mid = ((offset >> 16) & 0xFFFF) as u16;
        IDT[index as usize].offset_high = ((offset >> 32) & 0xFFFFFFFF) as u32;
        IDT[index as usize].selector = selector;
        IDT[index as usize].ist = ist & 0x7;
        IDT[index as usize].types_attr = types_attr;
        IDT[index as usize].reserved = 0;
    }
}

