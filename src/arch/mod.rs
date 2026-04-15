#[cfg(target_arch = "x86_64")]
pub mod cpu;
#[cfg(target_arch = "x86_64")]
pub mod idt;
#[cfg(target_arch = "x86_64")]
pub mod paging;

pub use cpu::*;
pub use idt::*;
pub use paging::*;

#[cfg(feature = "sse")]
pub const FEATURE_SSE: bool = true;
#[cfg(not(feature = "sse"))]
pub const FEATURE_SSE: bool = false;

#[cfg(feature = "apic")]
pub const FEATURE_APIC: bool = true;
#[cfg(not(feature = "apic"))]
pub const FEATURE_APIC: bool = false;

#[cfg(feature = "fpu")]
pub const FEATURE_FPU: bool = true;
#[cfg(not(feature = "fpu"))]
pub const FEATURE_FPU: bool = false;

pub const ARCH_BITS: u8 = 64;
pub const POINTER_SIZE: usize = 8;
pub const PAGE_SIZE: usize = 4096;
pub const PAGE_SHIFT: usize = 12;
pub const KERNEL_BASE: usize = 0xFFFF800000000000;
pub const USER_BASE: usize = 0x0000000000400000;

pub fn arch_init() {
    cpu::init();
}

#[macro_export]
macro_rules! arch_specific {
    ($x86_64:expr) => {
        $x86_64
    };
}

