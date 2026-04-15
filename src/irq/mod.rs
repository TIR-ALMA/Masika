pub mod apic;
pub mod handler;

use core::arch;

pub fn init() {
    apic::init();
    handler::init();
}

pub fn enable() {
    unsafe { arch::asm!("sti") };
}

pub fn disable() {
    unsafe { arch::asm!("cli") };
}

pub fn eoi(irq: u8) {
    apic::eoi(irq);
}

pub fn mask(irq: u8) {
    apic::mask_irq(irq);
}

pub fn unmask(irq: u8) {
    apic::unmask_irq(irq);
}

pub fn install_handler(irq: u8, handler: extern "C" fn()) {
    handler::install(irq, handler);
}

pub fn wait_for_interrupt() {
    unsafe { arch::asm!("hlt") };
}

