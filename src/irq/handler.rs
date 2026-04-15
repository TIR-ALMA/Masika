use crate::arch::cpu::{cli, sti, inb, outb};
use crate::console::kprintln;
use core::arch::asm;
use core::sync::atomic::{AtomicU32, Ordering};

static mut IRQ_COUNTS: [AtomicU32; 256] = [const { AtomicU32::new(0) }; 256];
static mut LATENCY_SAMPLES: [AtomicU32; 256] = [const { AtomicU32::new(0) }; 256];
static mut IRQ_ENABLED: bool = false;

pub struct IrqHandler {
    pub handler: fn(u8),
    pub name: &'static str,
}

static mut HANDLERS: [Option<IrqHandler>; 256] = [None; 256];

pub fn handle_irq(irq: u8) {
    unsafe {
        IRQ_COUNTS[irq as usize].fetch_add(1, Ordering::SeqCst);
    }

    if let Some(handler_info) = unsafe { &HANDLERS[irq as usize] } {
        (handler_info.handler)(irq);
    } else {
        default_irq_handler(irq);
    }

    if irq >= 40 {
        unsafe { outb(0xA0, 0x20) };
    }
    unsafe { outb(0x20, 0x20) };
}

fn default_irq_handler(irq: u8) {
    match irq {
        32 => handle_timer(),
        33 => handle_keyboard(),
        34 => handle_cascade(),
        35 => handle_com2(),
        36 => handle_com1(),
        37 => handle_lpt2(),
        38 => handle_floppy(),
        39 => handle_lpt1(),
        40 => handle_rtc(),
        41 => handle_pci1(),
        42 => handle_pci2(),
        43 => handle_pci3(),
        44 => handle_mouse(),
        45 => handle_fpu(),
        46 => handle_ide1(),
        47 => handle_ide2(),
        _ => {
            unsafe { outb(0x20, 0x20) };
        }
    }
}

fn handle_timer() {
    unsafe { outb(0x20, 0x20) };
}

fn handle_keyboard() {
    unsafe { outb(0x20, 0x20) };
}

fn handle_mouse() {
    unsafe { outb(0x20, 0x20) };
    unsafe { outb(0xA0, 0x20) };
}

fn handle_cascade() {
    unsafe { outb(0x20, 0x20) };
}

fn handle_com2() {
    unsafe { outb(0x20, 0x20) };
}

fn handle_com1() {
    unsafe { outb(0x20, 0x20) };
}

fn handle_lpt2() {
    unsafe { outb(0x20, 0x20) };
}

fn handle_floppy() {
    unsafe { outb(0x20, 0x20) };
}

fn handle_lpt1() {
    unsafe { outb(0x20, 0x20) };
}

fn handle_rtc() {
    unsafe { outb(0x20, 0x20) };
}

fn handle_pci1() {
    unsafe { outb(0x20, 0x20) };
}

fn handle_pci2() {
    unsafe { outb(0x20, 0x20) };
}

fn handle_pci3() {
    unsafe { outb(0x20, 0x20) };
}

fn handle_fpu() {
    unsafe { outb(0x20, 0x20) };
}

fn handle_ide1() {
    unsafe { outb(0x20, 0x20) };
}

fn handle_ide2() {
    unsafe { outb(0x20, 0x20) };
}

pub fn mask_irq(line: u8) {
    let port = if line < 8 { 0x21 } else { 0xA1 };
    let mask = unsafe { inb(port) };
    unsafe { outb(port, mask | (1 << (line & 7))) };
}

pub fn unmask_irq(line: u8) {
    let port = if line < 8 { 0x21 } else { 0xA1 };
    let mask = unsafe { inb(port) };
    unsafe { outb(port, mask & !(1 << (line & 7))) };
}

pub fn install_handlers() {
    cli();
    for i in 32..=255 {
        unmask_irq(i - 32);
    }
    unsafe { IRQ_ENABLED = true; }
    sti();
}

pub fn register_handler(irq: u8, handler: fn(u8), name: &'static str) {
    unsafe {
        HANDLERS[irq as usize] = Some(IrqHandler { handler, name });
    }
}

pub fn unregister_handler(irq: u8) {
    unsafe {
        HANDLERS[irq as usize] = None;
    }
}

#[no_mangle]
pub extern "C" fn rust_handle_irq(irq: u8) {
    handle_irq(irq);
}

pub fn get_irq_count(irq: u8) -> u32 {
    unsafe { IRQ_COUNTS[irq as usize].load(Ordering::SeqCst) }
}

pub fn reset_irq_counts() {
    for count in unsafe { &mut IRQ_COUNTS } {
        count.store(0, Ordering::SeqCst);
    }
}

pub fn enable_irq_global() {
    unsafe { IRQ_ENABLED = true; }
    sti();
}

pub fn disable_irq_global() {
    cli();
    unsafe { IRQ_ENABLED = false; }
}

pub fn is_irq_enabled() -> bool {
    unsafe { IRQ_ENABLED }
}

pub fn ack_spurious() {
    unsafe { outb(0x20, 0x20) };
}

pub fn handle_nmi() {
    kprintln!("NMI received");
}

pub fn measure_latency_start(irq: u8) {
    let tsc = unsafe { read_tsc() };
    unsafe {
        LATENCY_SAMPLES[irq as usize].store(tsc as u32, Ordering::SeqCst);
    }
}

pub fn measure_latency_end(irq: u8) {
    let tsc = unsafe { read_tsc() };
    let start = unsafe { LATENCY_SAMPLES[irq as usize].load(Ordering::SeqCst) };
    let latency = tsc.wrapping_sub(start as u64);
    unsafe {
        LATENCY_SAMPLES[irq as usize].store(latency as u32, Ordering::SeqCst);
    }
}

unsafe fn read_tsc() -> u64 {
    let low: u32;
    let high: u32;
    asm!(
        "rdtsc",
        out("eax") low,
        out("edx") high,
    );
    ((high as u64) << 32) | (low as u64)
}

pub fn get_irq_latency(irq: u8) -> u32 {
    unsafe { LATENCY_SAMPLES[irq as usize].load(Ordering::SeqCst) }
}

pub fn print_irq_stats() {
    for i in 0..256 {
        let count = get_irq_count(i as u8);
        if count > 0 {
            let latency = get_irq_latency(i as u8);
            kprintln!("IRQ {}: {} calls, {} cycles avg latency", i, count, latency);
        }
    }
}

pub fn clear_irq_stats() {
    for i in 0..256 {
        unsafe {
            IRQ_COUNTS[i].store(0, Ordering::SeqCst);
            LATENCY_SAMPLES[i].store(0, Ordering::SeqCst);
        }
    }
}

pub fn set_irq_affinity(irq: u8, cpu_mask: u32) {
    // Stub implementation
}

pub fn get_irq_affinity(irq: u8) -> u32 {
    1
}

pub fn request_irq_threaded(irq: u8, handler: fn(u8)) -> Result<(), ()> {
    register_handler(irq, handler, "threaded");
    Ok(())
}

pub fn free_irq(irq: u8) {
    unregister_handler(irq);
}

pub fn enable_irq(irq: u8) {
    unmask_irq(irq);
}

pub fn disable_irq(irq: u8) {
    mask_irq(irq);
}

pub fn local_irq_save() -> u64 {
    let flags: u64;
    unsafe {
        asm!("pushfq; pop {}", out(reg) flags);
    }
    cli();
    flags
}

pub fn local_irq_restore(flags: u64) {
    if (flags & 0x200) != 0 {
        sti();
    }
}

pub fn local_irq_disable() {
    cli();
}

pub fn local_irq_enable() {
    sti();
}

pub fn irq_exit() {
    unsafe { outb(0x20, 0x20) };
}

pub fn check_spurious(irq: u8) -> bool {
    if irq == 7 || irq == 15 {
        return true;
    }
    false
}

pub fn handle_error_irq() {
    kprintln!("IRQ Error");
}

pub fn handle_threshold_irq() {
    kprintln!("Threshold IRQ");
}

pub fn handle_corrected_irq() {
    kprintln!("Corrected IRQ");
}

pub fn handle_uncorrected_irq() {
    kprintln!("Uncorrected IRQ");
}

pub fn install_default_handlers() {
    for i in 32..48 {
        register_handler(i, default_irq_handler, "default");
    }
}

pub fn setup_irq_controller() {
    unsafe {
        outb(0x20, 0x11);
        outb(0x21, 0x20);
        outb(0x21, 0x04);
        outb(0x21, 0x01);
        outb(0xA0, 0x11);
        outb(0xA1, 0x28);
        outb(0xA1, 0x02);
        outb(0xA1, 0x01);
        outb(0x21, 0xFD);
        outb(0xA1, 0xFF);
    }
}

pub fn eoi_irq(irq: u8) {
    if irq >= 40 {
        unsafe { outb(0xA0, 0x20) };
    }
    unsafe { outb(0x20, 0x20) };
}

pub fn is_irq_pending(line: u8) -> bool {
    let port = if line < 8 { 0x20 } else { 0xA0 };
    let irr = unsafe { inb(port + 0x0A) };
    (irr & (1 << (line & 7))) != 0
}

pub fn wait_for_irq(irq: u8) {
    while !is_irq_pending(irq) {}
}

pub fn sync_irq(irq: u8) {
    cli();
    while is_irq_pending(irq) {
        sti();
        while is_irq_pending(irq) {}
        cli();
    }
    sti();
}

pub fn handle_level_triggered(irq: u8) {
    handle_irq(irq);
}

pub fn handle_edge_triggered(irq: u8) {
    handle_irq(irq);
}

pub fn set_irq_type(irq: u8, level: bool) {
    // Stub for level/edge trigger type
}

pub fn get_irq_type(irq: u8) -> bool {
    true
}

pub fn handle_nested_irq() {
    // Stub for nested interrupt handling
}

pub fn enter_irq_context() {
    cli();
}

pub fn exit_irq_context() {
    sti();
}

pub fn in_interrupt() -> bool {
    true
}

pub fn in_irq() -> bool {
    true
}

pub fn in_softirq() -> bool {
    false
}

pub fn in_hardirq() -> bool {
    true
}

pub fn handle_bad_irq(irq: u8) {
    kprintln!("Bad IRQ: {}", irq);
}

pub fn handle_fasteoi_irq(irq: u8) {
    unsafe { outb(0x20, 0x20) };
}

pub fn handle_per_cpu_irq(irq: u8) {
    handle_irq(irq);
}

pub fn handle_percpu_devid_irq(irq: u8) {
    handle_irq(irq);
}

pub fn handle_nested_thread_irq(irq: u8) {
    handle_irq(irq);
}

pub fn handle_nested_irq_with_ack(irq: u8) {
    handle_irq(irq);
    eoi_irq(irq);
}

pub fn handle_irq_work() {
    // Stub for deferred work
}

pub fn handle_irq_poll() {
    // Stub for polling IRQs
}

pub fn handle_irq_throttle() {
    // Stub for throttling logic
}

pub fn handle_irq_retrigger() {
    // Stub for retriggering logic
}

pub fn handle_irq_resend() {
    // Stub for resend logic
}

pub fn handle_irq_coalesce() {
    // Stub for coalescing logic
}

pub fn handle_irq_delayed() {
    // Stub for delayed handling
}

pub fn handle_irq_batched() {
    // Stub for batched handling
}

pub fn handle_irq_aggregated() {
    // Stub for aggregated handling
}

pub fn handle_irq_filtered() {
    // Stub for filtered handling
}

pub fn handle_irq_masked() {
    // Stub for masked handling
}

pub fn handle_irq_unmasked() {
    // Stub for unmasked handling
}

pub fn handle_irq_suspended() {
    // Stub for suspended handling
}

pub fn handle_irq_resumed() {
    // Stub for resumed handling
}

pub fn handle_irq_offline() {
    // Stub for offline handling
}

pub fn handle_irq_online() {
    // Stub for online handling
}

pub fn handle_irq_startup() {
    // Stub for startup handling
}

pub fn handle_irq_shutdown() {
    // Stub for shutdown handling
}

pub fn handle_irq_enable() {
    // Stub for enable handling
}

pub fn handle_irq_disable() {
    // Stub for disable handling
}

pub fn handle_irq_set_affinity() {
    // Stub for affinity setting
}

pub fn handle_irq_retrigger_irq() {
    // Stub for retrigger
}

pub fn handle_irq_sync() {
    // Stub for sync
}

pub fn handle_irq_no_debug() {
    // Stub for no-debug handling
}

pub fn handle_irq_wake_thread() {
    // Stub for thread wake
}

pub fn handle_irq_managed_shutdown() {
    // Stub for managed shutdown
}

pub fn handle_irq_force_secondary() {
    // Stub for secondary handling
}

pub fn handle_irq_chip_retrigger() {
    // Stub for chip retrigger
}

pub fn handle_irq_chip_set_affinity() {
    // Stub for chip affinity
}

pub fn handle_irq_chip_bus_lock() {
    // Stub for bus lock
}

pub fn handle_irq_chip_bus_sync_unlock() {
    // Stub for bus unlock
}

pub fn handle_irq_nested_primary() {
    // Stub for nested primary
}

pub fn handle_irq_nested_secondary() {
    // Stub for nested secondary
}

pub fn handle_irq_nested_thread() {
    // Stub for nested thread
}

pub fn handle_irq_nested_throttle() {
    // Stub for nested throttle
}

pub fn handle_irq_nested_coalesce() {
    // Stub for nested coalesce
}

pub fn handle_irq_nested_delayed() {
    // Stub for nested delayed
}

pub fn handle_irq_nested_batched() {
    // Stub for nested batched
}

pub fn handle_irq_nested_aggregated() {
    // Stub for nested aggregated
}

pub fn handle_irq_nested_filtered() {
    // Stub for nested filtered
}

pub fn handle_irq_nested_masked() {
    // Stub for nested masked
}

pub fn handle_irq_nested_unmasked() {
    // Stub for nested unmasked
}

pub fn handle_irq_nested_suspended() {
    // Stub for nested suspended
}

pub fn handle_irq_nested_resumed() {
    // Stub for nested resumed
}

pub fn handle_irq_nested_offline() {
    // Stub for nested offline
}

pub fn handle_irq_nested_online() {
    // Stub for nested online
}

pub fn handle_irq_nested_startup() {
    // Stub for nested startup
}

pub fn handle_irq_nested_shutdown() {
    // Stub for nested shutdown
}

pub fn handle_irq_nested_enable() {
    // Stub for nested enable
}

pub fn handle_irq_nested_disable() {
    // Stub for nested disable
}

pub fn handle_irq_nested_set_affinity() {
    // Stub for nested affinity
}

pub fn handle_irq_nested_retrigger_irq() {
    // Stub for nested retrigger
}

pub fn handle_irq_nested_sync() {
    // Stub for nested sync
}

pub fn handle_irq_nested_no_debug() {
    // Stub for nested no-debug
}

pub fn handle_irq_nested_wake_thread() {
    // Stub for nested thread wake
}

pub fn handle_irq_nested_managed_shutdown() {
    // Stub for nested managed shutdown
}

