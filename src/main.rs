#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(global_asm)]
#![feature(asm_const)]
#![feature(naked_functions)]

use core::panic::PanicInfo;

mod arch;
mod memory;
mod scheduler;
mod console;
mod irq;
mod syscall;
mod sync;
mod timer;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        arch::cpu::early_setup();
    }

    memory::init();

    console::init();
    console::print_str("Kernel started\n");

    arch::idt::init_idt();
    console::print_str("IDT initialized\n");

    arch::cpu::setup_fpu();
    console::print_str("FPU enabled\n");

    arch::paging::init_paging();
    console::print_str("Paging initialized\n");

    timer::init_timer();
    console::print_str("Timer initialized\n");

    irq::apic::init_apic();
    console::print_str("APIC initialized\n");

    scheduler::init_scheduler();
    console::print_str("Scheduler initialized\n");

    syscall::init_syscalls();
    console::print_str("Syscalls initialized\n");

    unsafe {
        arch::cpu::enable_interrupts();
    }

    console::print_str("Enabling interrupts...\n");

    main_loop()
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    _start()
}

fn main_loop() -> ! {
    loop {
        unsafe {
            arch::cpu::halt();
        }
    }
}

static mut PANIC_OCCURRED: bool = false;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if unsafe { PANIC_OCCURRED } {
        loop {
            unsafe {
                arch::cpu::halt();
            }
        }
    }

    unsafe {
        PANIC_OCCURRED = true;
        arch::cpu::disable_interrupts();
    }

    console::error_println!("KERNEL PANIC OCCURRED");
    console::error_println!("{}", info);

    if let Some(location) = info.location() {
        console::error_println!("File: {}, Line: {}", location.file(), location.line());
    }

    arch::cpu::dump_registers();

    loop {
        unsafe {
            arch::cpu::halt();
        }
    }
}

pub fn kernel_main() -> ! {
    _start()
}

pub fn halt_cpu() -> ! {
    loop {
        unsafe {
            arch::cpu::halt();
        }
    }
}

pub fn enable_interrupts_safe() {
    unsafe {
        arch::cpu::enable_interrupts();
    }
}

pub fn disable_interrupts_safe() {
    unsafe {
        arch::cpu::disable_interrupts();
    }
}

pub fn get_kernel_base() -> usize {
    0xFFFF800000000000
}

pub fn get_current_stack_pointer() -> usize {
    let rsp: usize;
    unsafe {
        core::arch::asm!("mov {}, rsp", out(reg) rsp);
    }
    rsp
}

pub fn get_current_frame_pointer() -> usize {
    let rbp: usize;
    unsafe {
        core::arch::asm!("mov {}, rbp", out(reg) rbp);
    }
    rbp
}

pub fn kernel_heap_start() -> usize {
    0xFFFF800010000000
}

pub fn kernel_heap_end() -> usize {
    0xFFFF800020000000
}

pub fn kernel_code_start() -> usize {
    0xFFFFFFFF80000000
}

pub fn kernel_code_end() -> usize {
    0xFFFFFFFF80200000
}

pub fn kernel_data_start() -> usize {
    0xFFFFFFFF80200000
}

pub fn kernel_data_end() -> usize {
    0xFFFFFFFF80400000
}

pub fn get_boot_info() -> *const u8 {
    0x7000 as *const u8
}

pub fn wait_cycles(cycles: u64) {
    let start = arch::cpu::rdtsc();
    while arch::cpu::rdtsc() - start < cycles {}
}

pub fn kernel_version() -> &'static str {
    "V"
}

pub fn kernel_name() -> &'static str {
    "Masika"
}

pub fn print_welcome_message() {
    console::print_str("Welcome to ");
    console::print_str(kernel_name());
    console::print_str(" v");
    console::print_str(kernel_version());
    console::print_str("\n");
}

pub fn initialize_subsystems() {
    print_welcome_message();
}

pub fn post_init_tasks() {
    console::print_str("All systems operational\n");
}

pub fn check_hardware_capabilities() {
    let cpuid = arch::cpu::get_cpuid(1, 0);
    let edx = (cpuid.edx >> 25) & 1;
    if edx != 0 {
        console::print_str("SSE supported\n");
    } else {
        console::error_println!("SSE not supported!");
    }
}

pub fn setup_multicore() {
    console::print_str("Initializing multicore support\n");
}

pub fn finalize_boot_process() {
    console::print_str("Boot process completed\n");
}

pub fn enter_idle_state() -> ! {
    loop {
        unsafe {
            arch::cpu::halt();
        }
    }
}

pub fn kernel_entry_point() -> ! {
    _start()
}

