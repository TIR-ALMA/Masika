use core::arch::asm;
use core::ptr;

use crate::memory::virtual::is_user_addr;
use crate::syscall::core::*;
use crate::syscall::extra::*;
use crate::syscall::{SyscallNum, SysResult};

static SYSCALL_TABLE: SyscallTable = SyscallTable::new();

#[repr(C)]
pub struct UserBuffer {
    ptr: u64,
    size: usize,
}

pub struct SyscallTable {
    handlers: [Option<unsafe extern "C" fn(u64, u64, u64, u64, u64, u64) -> u64>; 256],
}

impl SyscallTable {
    pub const fn new() -> Self {
        const DEFAULT: Option<unsafe extern "C" fn(u64, u64, u64, u64, u64, u64) -> u64> = None;
        let mut table = [DEFAULT; 256];
        table[SyscallNum::READ as usize] = Some(sys_read);
        table[SyscallNum::WRITE as usize] = Some(sys_write);
        table[SyscallNum::EXIT as usize] = Some(sys_exit);
        table[SyscallNum::MMAP as usize] = Some(sys_mmap);
        table[SyscallNum::BRK as usize] = Some(sys_brk);
        table[SyscallNum::EXECVE as usize] = Some(sys_execve);
        table[SyscallNum::GETPID as usize] = Some(sys_getpid);
        table[SyscallNum::GETTID as usize] = Some(sys_gettid);
        table[SyscallNum::CLOCK_GETTIME as usize] = Some(sys_clock_gettime);
        table[SyscallNum::FUTEX as usize] = Some(sys_futex);
        table[SyscallNum::STAT as usize] = Some(sys_stat);
        table[SyscallNum::OPEN as usize] = Some(sys_open);
        table[SyscallNum::CLOSE as usize] = Some(sys_close);
        table[SyscallNum::DUP as usize] = Some(sys_dup);
        table[SyscallNum::PIPE as usize] = Some(sys_pipe);
        table[SyscallNum::WAIT4 as usize] = Some(sys_wait4);
        table[SyscallNum::KILL as usize] = Some(sys_kill);
        table[SyscallNum::SIGACTION as usize] = Some(sys_sigaction);
        table[SyscallNum::RT_SIGPROCMASK as usize] = Some(sys_rt_sigprocmask);
        table[SyscallNum::POLL as usize] = Some(sys_poll);
        table[SyscallNum::IOCTL as usize] = Some(sys_ioctl);
        table[SyscallNum::CLONE as usize] = Some(sys_clone);
        table[SyscallNum::NANOSLEEP as usize] = Some(sys_nanosleep);
        table[SyscallNum::SET_TID_ADDRESS as usize] = Some(sys_set_tid_address);
        table[SyscallNum::ARCH_PRCTL as usize] = Some(sys_arch_prctl);
        table[SyscallNum::GETDENTS64 as usize] = Some(sys_getdents64);
        table[SyscallNum::MPROTECT as usize] = Some(sys_mprotect);
        table[SyscallNum::MUNMAP as usize] = Some(sys_munmap);
        table[SyscallNum::SYSINFO as usize] = Some(sys_sysinfo);
        Self { handlers: table }
    }

    pub unsafe fn dispatch(&self, num: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) -> u64 {
        if num >= 256 { return -1i64 as u64; }
        match self.handlers[num as usize] {
            Some(handler) => {
                let res = handler(a1, a2, a3, a4, a5, a6);
                res
            }
            None => -1i64 as u64,
        }
    }
}

pub unsafe fn copy_from_user(dst: *mut u8, src: *const u8, len: usize) -> SysResult<()> {
    if !is_user_addr(src as u64) || !is_user_addr((src as u64).wrapping_add(len as u64)) { return Err(-1); }
    let mut i = 0;
    while i < len {
        let src_ptr = (src as u64).wrapping_add(i as u64) as *const u8;
        if !is_user_addr(src_ptr as u64) { return Err(-1); }
        unsafe { ptr::write_volatile(dst.add(i), ptr::read_volatile(src_ptr)); }
        i += 1;
    }
    Ok(())
}

pub unsafe fn copy_to_user(dst: *mut u8, src: *const u8, len: usize) -> SysResult<()> {
    if !is_user_addr(dst as u64) || !is_user_addr((dst as u64).wrapping_add(len as u64)) { return Err(-1); }
    let mut i = 0;
    while i < len {
        let dst_ptr = (dst as u64).wrapping_add(i as u64) as *mut u8;
        if !is_user_addr(dst_ptr as u64) { return Err(-1); }
        unsafe { ptr::write_volatile(dst_ptr, ptr::read_volatile(src.add(i))); }
        i += 1;
    }
    Ok(())
}

pub fn validate_user_ptr(ptr: u64, size: u64) -> bool {
    if ptr == 0 { return false; }
    if size == 0 { return true; }
    let end = ptr.wrapping_add(size);
    if end < ptr { return false; }
    is_user_addr(ptr) && is_user_addr(end)
}

pub fn handle_syscall(num: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) -> u64 {
    unsafe { SYSCALL_TABLE.dispatch(num, a1, a2, a3, a4, a5, a6) }
}

#[inline(always)]
pub fn check_access_ok(addr: u64, size: u64) -> bool {
    validate_user_ptr(addr, size)
}

pub fn copy_from_user_array<T>(dst: &mut [T], src: *const T) -> SysResult<()> {
    if dst.is_empty() { return Ok(()); }
    let byte_size = dst.len() * core::mem::size_of::<T>();
    if !validate_user_ptr(src as u64, byte_size as u64) { return Err(-1); }
    unsafe {
        copy_from_user(dst.as_mut_ptr() as *mut u8, src as *const u8, byte_size)?;
    }
    Ok(())
}

pub fn copy_to_user_array<T>(dst: *mut T, src: &[T]) -> SysResult<()> {
    if src.is_empty() { return Ok(()); }
    let byte_size = src.len() * core::mem::size_of::<T>();
    if !validate_user_ptr(dst as u64, byte_size as u64) { return Err(-1); }
    unsafe {
        copy_to_user(dst as *mut u8, src.as_ptr() as *const u8, byte_size)?;
    }
    Ok(())
}

pub fn get_user_u64(ptr: *const u64) -> SysResult<u64> {
    if !validate_user_ptr(ptr as u64, 8) { return Err(-1); }
    let mut val = 0u64;
    unsafe {
        copy_from_user(&mut val as *mut u64 as *mut u8, ptr as *const u8, 8)?;
    }
    Ok(val)
}

pub fn put_user_u64(ptr: *mut u64, val: u64) -> SysResult<()> {
    if !validate_user_ptr(ptr as u64, 8) { return Err(-1); }
    unsafe {
        copy_to_user(ptr as *mut u8, &val as *const u64 as *const u8, 8)?;
    }
    Ok(())
}

pub fn validate_string_ptr(ptr: *const u8) -> SysResult<usize> {
    let mut len = 0;
    loop {
        if len > 4096 { return Err(-1); }
        if !validate_user_ptr(ptr as u64 + len as u64, 1) { return Err(-1); }
        let mut byte = 0u8;
        unsafe {
            copy_from_user(&mut byte as *mut u8, (ptr as u64 + len as u64) as *const u8, 1)?;
        }
        if byte == 0 { break; }
        len += 1;
    }
    Ok(len)
}

pub fn copy_string_from_user(dst: &mut [u8], src: *const u8) -> SysResult<usize> {
    let max_len = dst.len().min(4096);
    let mut i = 0;
    while i < max_len {
        if !validate_user_ptr(src as u64 + i as u64, 1) { return Err(-1); }
        let mut byte = 0u8;
        unsafe {
            copy_from_user(&mut byte as *mut u8, (src as u64 + i as u64) as *const u8, 1)?;
        }
        if byte == 0 { break; }
        dst[i] = byte;
        i += 1;
    }
    Ok(i)
}

pub fn copy_string_to_user(dst: *mut u8, src: &[u8]) -> SysResult<()> {
    let len = src.len();
    if !validate_user_ptr(dst as u64, len as u64) { return Err(-1); }
    if len == 0 { return Ok(()); }
    unsafe {
        copy_to_user(dst, src.as_ptr(), len)?;
    }
    Ok(())
}

pub fn handle_error(result: SysResult<u64>) -> u64 {
    match result {
        Ok(value) => value,
        Err(errno) => errno as u64,
    }
}

pub fn validate_iovec(vec: *const IoVec, count: usize) -> SysResult<()> {
    if count == 0 { return Ok(()); }
    if count > 1024 { return Err(-1); }
    if !validate_user_ptr(vec as u64, (count * core::mem::size_of::<IoVec>()) as u64) { return Err(-1); }
    for i in 0..count {
        let iov = unsafe { &*vec.add(i) };
        if !validate_user_ptr(iov.base as u64, iov.len as u64) { return Err(-1); }
    }
    Ok(())
}

#[repr(C)]
pub struct IoVec {
    pub base: *mut u8,
    pub len: usize,
}

pub fn copy_iovec_from_user(dst: &mut [IoVec], src: *const IoVec) -> SysResult<()> {
    if dst.is_empty() { return Ok(()); }
    let byte_size = dst.len() * core::mem::size_of::<IoVec>();
    if !validate_user_ptr(src as u64, byte_size as u64) { return Err(-1); }
    unsafe {
        copy_from_user(
            dst.as_mut_ptr() as *mut u8,
            src as *const u8,
            byte_size
        )?;
    }
    Ok(())
}

pub fn copy_iovec_to_user(dst: *mut IoVec, src: &[IoVec]) -> SysResult<()> {
    if src.is_empty() { return Ok(()); }
    let byte_size = src.len() * core::mem::size_of::<IoVec>();
    if !validate_user_ptr(dst as u64, byte_size as u64) { return Err(-1); }
    unsafe {
        copy_to_user(dst as *mut u8, src.as_ptr() as *const u8, byte_size)?;
    }
    Ok(())
}

pub fn validate_user_range(start: u64, size: u64) -> bool {
    if size == 0 { return true; }
    if start == 0 { return false; }
    let end = start.wrapping_add(size);
    if end < start { return false; }
    is_user_addr(start) && is_user_addr(end)
}

pub fn copy_page_aligned_from_user(dst: &mut [u8], src: *const u8) -> SysResult<()> {
    let size = dst.len();
    if size == 0 { return Ok(()); }
    let aligned_start = src as u64 & !0xFFF;
    let aligned_end = (src as u64 + size as u64 + 0xFFF) & !0xFFF;
    if !is_user_addr(aligned_start) || !is_user_addr(aligned_end) { return Err(-1); }
    
    if !validate_user_ptr(src as u64, size as u64) { return Err(-1); }
    unsafe {
        copy_from_user(dst.as_mut_ptr(), src, size)?;
    }
    Ok(())
}

pub fn copy_page_aligned_to_user(dst: *mut u8, src: &[u8]) -> SysResult<()> {
    let size = src.len();
    if size == 0 { return Ok(()); }
    let aligned_start = dst as u64 & !0xFFF;
    let aligned_end = (dst as u64 + size as u64 + 0xFFF) & !0xFFF;
    if !is_user_addr(aligned_start) || !is_user_addr(aligned_end) { return Err(-1); }
    
    if !validate_user_ptr(dst as u64, size as u64) { return Err(-1); }
    unsafe {
        copy_to_user(dst, src.as_ptr(), size)?;
    }
    Ok(())
}

pub fn copy_from_user_with_limit(src: *const u8, limit: usize) -> SysResult<Vec<u8>> {
    let mut len = 0;
    while len < limit {
        if !validate_user_ptr(src as u64 + len as u64, 1) { return Err(-1); }
        let mut byte = 0u8;
        unsafe {
            copy_from_user(&mut byte as *mut u8, (src as u64 + len as u64) as *const u8, 1)?;
        }
        if byte == 0 { break; }
        len += 1;
    }
    if len == limit { return Err(-1); }
    let mut vec = Vec::with_capacity(len);
    unsafe {
        vec.set_len(len);
        copy_from_user(vec.as_mut_ptr(), src, len)?;
    }
    Ok(vec)
}

pub fn validate_and_get_user_slice<T>(ptr: *const T, count: usize) -> SysResult<&'static [T]> {
    if count == 0 { return Ok(&[]); }
    if count > 1024 { return Err(-1); }
    let byte_size = count * core::mem::size_of::<T>();
    if !validate_user_ptr(ptr as u64, byte_size as u64) { return Err(-1); }
    Ok(unsafe { core::slice::from_raw_parts(ptr, count) })
}

