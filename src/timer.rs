use core::arch::x86_64::{__rdtsc, _mm_mfence};
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};

static TICKS: AtomicU64 = AtomicU64::new(0);
static FREQUENCY: AtomicU64 = AtomicU64::new(0);
static TSC_KNOWN: AtomicBool = AtomicBool::new(false);
static mut PIT_BASE_FREQ: u64 = 1193182;

#[derive(Clone, Copy)]
pub struct TimeVal {
    pub sec: u64,
    pub usec: u64,
}

#[derive(Clone, Copy)]
pub struct ITimerSpec {
    pub it_interval: TimeVal,
    pub it_value: TimeVal,
}

#[repr(C)]
struct HPET {
    cap_id: u64,
    config: u64,
    isr: u64,
    _rsv: [u64; 25],
    main_ctr: u64,
}

const HPET_BASE: *mut HPET = 0xFED00000 as *mut HPET;
const PIT_PORT_CMD: u16 = 0x43;
const PIT_PORT_DATA: u16 = 0x40;

#[inline]
fn outb(port: u16, val: u8) {
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack)) };
}

#[inline]
fn inb(port: u16) -> u8 {
    let ret: u8;
    unsafe { core::arch::asm!("in al, dx", in("dx") port, out("al") ret, options(nomem, nostack)) };
    ret
}

#[inline]
fn outw(port: u16, val: u16) {
    unsafe { core::arch::asm!("out dx, ax", in("dx") port, in("ax") val, options(nomem, nostack)) };
}

#[inline]
fn inw(port: u16) -> u16 {
    let ret: u16;
    unsafe { core::arch::asm!("in ax, dx", in("dx") port, out("ax") ret, options(nomem, nostack)) };
    ret
}

pub fn init() {
    if is_hpet_capable() {
        if init_hpet() {
            return;
        }
    }

    init_pit();
    calibrate_tsc();
}

fn is_hpet_capable() -> bool {
    let hpet = unsafe { &*HPET_BASE };
    hpet.cap_id != 0 && (hpet.cap_id & (1 << 13)) != 0
}

unsafe fn read_hpet_reg(offset: usize) -> u64 {
    let ptr = (HPET_BASE as *const u8).add(offset) as *const u64;
    ptr.read_volatile()
}

unsafe fn write_hpet_reg(offset: usize, value: u64) {
    let ptr = (HPET_BASE as *mut u8).add(offset) as *mut u64;
    ptr.write_volatile(value);
}

fn init_hpet() -> bool {
    let hpet = unsafe { &mut *HPET_BASE };
    let period = (hpet.cap_id >> 32) as u32;
    if period == 0 {
        return false;
    }

    hpet.config &= !1;
    hpet.main_ctr = 0;
    hpet.config |= 1;

    let freq = 1_000_000_000_000_000_u64 / (period as u64);
    FREQUENCY.store(freq, Ordering::SeqCst);
    true
}

fn init_pit() {
    let divisor = 65535;
    outb(PIT_PORT_CMD, 0x36);
    outb(PIT_PORT_DATA, divisor as u8);
    outb(PIT_PORT_DATA, (divisor >> 8) as u8);
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::SeqCst)
}

pub fn uptime() -> u64 {
    let t = TICKS.load(Ordering::SeqCst);
    let f = FREQUENCY.load(Ordering::SeqCst);
    if f > 0 { t * 1_000_000_000 / f } else { 0 }
}

pub fn uptime_ms() -> u64 {
    let t = TICKS.load(Ordering::SeqCst);
    let f = FREQUENCY.load(Ordering::SeqCst);
    if f > 0 { t * 1_000_000 / f } else { 0 }
}

pub fn uptime_us() -> u64 {
    let t = TICKS.load(Ordering::SeqCst);
    let f = FREQUENCY.load(Ordering::SeqCst);
    if f > 0 { t * 1_000 / f } else { 0 }
}

pub fn calibrate_tsc() {
    if TSC_KNOWN.load(Ordering::Relaxed) {
        return;
    }

    let start = unsafe { __rdtsc() };
    let ticks_start = TICKS.load(Ordering::Relaxed);
    loop {
        let ticks_now = TICKS.load(Ordering::Relaxed);
        if ticks_now - ticks_start >= 100 {
            break;
        }
    }
    let end = unsafe { __rdtsc() };
    let delta = end - start;
    FREQUENCY.store(delta * 10, Ordering::SeqCst);
    TSC_KNOWN.store(true, Ordering::Relaxed);
}

pub fn sleep_ns(ns: u64) {
    let target = uptime() + ns;
    while uptime() < target {
        unsafe { core::arch::asm!("pause") };
    }
}

pub fn sleep_ms(ms: u64) {
    sleep_ns(ms * 1_000_000);
}

pub fn sleep_us(us: u64) {
    sleep_ns(us * 1_000);
}

pub fn sleep_ticks(ticks: u64) {
    let current = TICKS.load(Ordering::SeqCst);
    while TICKS.load(Ordering::SeqCst) < current + ticks {
        unsafe { core::arch::asm!("pause") };
    }
}

pub fn get_cycles() -> u64 {
    unsafe { __rdtsc() }
}

pub fn get_timeval() -> TimeVal {
    let ns = uptime();
    TimeVal {
        sec: ns / 1_000_000_000,
        usec: (ns % 1_000_000_000) / 1_000,
    }
}

pub fn delay_us(us: u64) {
    let cycles = us * FREQUENCY.load(Ordering::SeqCst) / 1_000_000;
    let start = unsafe { __rdtsc() };
    while unsafe { __rdtsc() } - start < cycles {
        unsafe { core::arch::asm!("pause") };
    }
}

pub fn tick() {
    TICKS.fetch_add(1, Ordering::SeqCst);
    _mm_mfence();
}

pub fn reset_timer() {
    TICKS.store(0, Ordering::SeqCst);
}

pub fn frequency() -> u64 {
    FREQUENCY.load(Ordering::SeqCst)
}

pub fn cycles_to_ns(cycles: u64) -> u64 {
    let freq = FREQUENCY.load(Ordering::SeqCst);
    if freq > 0 { cycles * 1_000_000_000 / freq } else { 0 }
}

pub fn ns_to_cycles(ns: u64) -> u64 {
    let freq = FREQUENCY.load(Ordering::SeqCst);
    if freq > 0 { ns * freq / 1_000_000_000 } else { 0 }
}

pub fn time_since(start_ns: u64) -> u64 {
    let now = uptime();
    if now >= start_ns {
        now - start_ns
    } else {
        0
    }
}

pub fn busy_wait_cycles(cycles: u64) {
    let start = unsafe { __rdtsc() };
    while unsafe { __rdtsc() } - start < cycles {
        core::hint::spin_loop();
    }
}

pub fn timer_enabled() -> bool {
    FREQUENCY.load(Ordering::SeqCst) > 0
}

pub fn wait_for_event<F>(timeout_ns: u64, mut condition: F) -> bool
where
    F: FnMut() -> bool,
{
    let start = uptime();
    while uptime() - start < timeout_ns {
        if condition() {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

pub fn microsecond_delay(us: u64) {
    let freq = FREQUENCY.load(Ordering::SeqCst);
    let cycles = (freq * us) / 1_000_000;
    busy_wait_cycles(cycles);
}

pub fn millisecond_delay(ms: u64) {
    microsecond_delay(ms * 1000);
}

pub fn precise_sleep_ns(ns: u64) {
    let start = uptime();
    let target = start + ns;
    let freq = FREQUENCY.load(Ordering::SeqCst);
    if freq > 0 {
        let cycles_per_ns = freq / 1_000_000_000;
        let remaining_ns = target - uptime();
        if remaining_ns > 0 && cycles_per_ns > 0 {
            let cycles = remaining_ns * cycles_per_ns;
            let start_cycle = unsafe { __rdtsc() };
            while unsafe { __rdtsc() } - start_cycle < cycles {
                core::hint::spin_loop();
            }
        }
    }
}

pub fn get_raw_counter() -> u64 {
    if is_hpet_capable() {
        unsafe { read_hpet_reg(0xF0) }
    } else {
        unsafe { __rdtsc() }
    }
}

pub fn tick_rate_hz() -> u64 {
    let freq = FREQUENCY.load(Ordering::SeqCst);
    if freq > 0 { freq / 1_000_000_000 } else { 100 }
}

pub fn elapsed_time(start: u64) -> u64 {
    let current = uptime();
    if current >= start {
        current - start
    } else {
        0
    }
}

pub fn timestamp_ns() -> u64 {
    uptime()
}

pub fn timestamp_us() -> u64 {
    uptime_us()
}

pub fn timestamp_ms() -> u64 {
    uptime_ms()
}

pub fn timer_interrupt_handler() {
    tick();
}

pub fn enable_periodic_mode() {
    outb(PIT_PORT_CMD, 0x36);
}

pub fn disable_timer() {
    if is_hpet_capable() {
        unsafe {
            write_hpet_reg(0x10, 0);
        }
    }
}

pub fn timer_frequency_detect() -> u64 {
    let base_freq = unsafe { PIT_BASE_FREQ };
    base_freq
}

pub fn wait_until_ns(target_ns: u64) {
    while uptime() < target_ns {
        core::hint::spin_loop();
    }
}

pub fn sleep_until_ns(target_ns: u64) {
    while uptime() < target_ns {
        unsafe { core::arch::asm!("pause") };
    }
}

pub fn get_boot_time_ns() -> u64 {
    0
}

pub fn timer_reset_frequency(freq: u64) {
    FREQUENCY.store(freq, Ordering::SeqCst);
}

pub fn convert_timestamp_to_date(timestamp: u64) -> (u32, u8, u8, u8, u8, u8) {
    const SECONDS_PER_DAY: u64 = 86400;
    const SECONDS_PER_HOUR: u64 = 3600;
    const SECONDS_PER_MINUTE: u64 = 60;
    
    let seconds = timestamp / 1_000_000_000;
    let days = seconds / SECONDS_PER_DAY;
    
    let year = 1970 + (days / 365) as u32;
    let month = ((days % 365) / 30) as u8 + 1;
    let day = ((days % 365) % 30) as u8 + 1;
    let hour = ((seconds % SECONDS_PER_DAY) / SECONDS_PER_HOUR) as u8;
    let minute = ((seconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE) as u8;
    let second = (seconds % SECONDS_PER_MINUTE) as u8;
    
    (year, month, day, hour, minute, second)
}

pub fn set_timer_callback<F>(_callback: F) where F: Fn() {}

