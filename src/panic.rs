use core::fmt::{self, Write};
use core::panic::PanicInfo;

struct Writer;

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        unsafe { log_bytes(s.as_ptr(), s.len()) };
        Ok(())
    }
}

extern "C" {
    fn log_bytes(ptr: *const u8, len: usize);
    fn halt();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let _ = writeln!(Writer, "\nPANIC: {}", info);
    dump_regs();
    loop {
        unsafe { halt() };
    }
}

#[repr(C)]
struct Regs {
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    rsp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    ss: u64,
}

fn dump_regs() {
    let mut regs: Regs = Regs {
        rax: 0, rbx: 0, rcx: 0, rdx: 0,
        rsi: 0, rdi: 0, rbp: 0, rsp: 0,
        r8: 0, r9: 0, r10: 0, r11: 0,
        r12: 0, r13: 0, r14: 0, r15: 0,
        rip: 0, cs: 0, rflags: 0, ss: 0,
    };

    unsafe {
        asm!(
            "mov {tmp}, rax",
            "pushfq; pop {tmp}",
            tmp = out(reg) regs.rflags,
            options(preserves_flags)
        );
        asm!("mov {}, cs", out(reg) regs.cs);
        asm!("mov {}, ss", out(reg) regs.ss);
        asm!("mov {}, rip", out(reg) regs.rip);
        asm!("mov {}, rsp", out(reg) regs.rsp);
        asm!("mov {}, rbp", out(reg) regs.rbp);
        asm!("mov {}, rax", out(reg) regs.rax);
        asm!("mov {}, rbx", out(reg) regs.rbx);
        asm!("mov {}, rcx", out(reg) regs.rcx);
        asm!("mov {}, rdx", out(reg) regs.rdx);
        asm!("mov {}, rsi", out(reg) regs.rsi);
        asm!("mov {}, rdi", out(reg) regs.rdi);
        asm!("mov {}, r8", out(reg) regs.r8);
        asm!("mov {}, r9", out(reg) regs.r9);
        asm!("mov {}, r10", out(reg) regs.r10);
        asm!("mov {}, r11", out(reg) regs.r11);
        asm!("mov {}, r12", out(reg) regs.r12);
        asm!("mov {}, r13", out(reg) regs.r13);
        asm!("mov {}, r14", out(reg) regs.r14);
        asm!("mov {}, r15", out(reg) regs.r15);
    }

    let _ = writeln!(Writer, "RIP: {:016x}", regs.rip);
    let _ = writeln!(Writer, "RSP: {:016x}", regs.rsp);
    let _ = writeln!(Writer, "RBP: {:016x}", regs.rbp);
    let _ = writeln!(Writer, "CS:  {:016x} SS:  {:016x}", regs.cs, regs.ss);
    let _ = writeln!(Writer, "RAX: {:016x} RBX: {:016x}", regs.rax, regs.rbx);
    let _ = writeln!(Writer, "RCX: {:016x} RDX: {:016x}", regs.rcx, regs.rdx);
    let _ = writeln!(Writer, "RSI: {:016x} RDI: {:016x}", regs.rsi, regs.rdi);
    let _ = writeln!(Writer, "R8:  {:016x} R9:  {:016x}", regs.r8, regs.r9);
    let _ = writeln!(Writer, "R10: {:016x} R11: {:016x}", regs.r10, regs.r11);
    let _ = writeln!(Writer, "R12: {:016x} R13: {:016x}", regs.r12, regs.r13);
    let _ = writeln!(Writer, "R14: {:016x} R15: {:016x}", regs.r14, regs.r15);
    let _ = writeln!(Writer, "RFLAGS: {:016x}", regs.rflags);

    let _ = writeln!(Writer, "--- Stack trace ---");
    let mut ptr = regs.rsp;
    for i in 0..10 {
        if let Some(val) = safe_read_ptr(ptr) {
            let _ = writeln!(Writer, "[{:2}] {:016x}", i, val);
        } else {
            break;
        }
        ptr += 8;
    }
}

unsafe fn safe_read_ptr(addr: u64) -> Option<u64> {
    if addr % 8 != 0 || addr < 0x100000 {
        return None;
    }
    Some(core::ptr::read_volatile(addr as *const u64))
}

