use core::ffi::c_void;
use core::ptr;

use crate::scheduler::pcb::ProcessId;
use crate::syscall::dispatch::UserPtr;
use crate::syscall::{SysResult, SysError};

#[repr(C)]
pub struct TimeSpec {
    tv_sec: i64,
    tv_nsec: i64,
}

#[repr(C)]
pub struct Stat {
    st_dev: u64,
    st_ino: u64,
    st_nlink: u32,
    st_mode: u32,
    st_uid: u32,
    st_gid: u32,
    __pad0: u32,
    st_rdev: u64,
    st_size: i64,
    st_blksize: i64,
    st_blocks: i64,
    st_atim: TimeSpec,
    st_mtim: TimeSpec,
    st_ctim: TimeSpec,
    __unused: [i64; 3],
}

#[repr(C)]
pub struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

#[repr(C)]
pub struct IoVec {
    iov_base: *mut c_void,
    iov_len: usize,
}

#[repr(C)]
pub struct UtsName {
    sysname: [u8; 65],
    nodename: [u8; 65],
    release: [u8; 65],
    version: [u8; 65],
    machine: [u8; 65],
    domainname: [u8; 65],
}

#[repr(C)]
pub struct SysInfo {
    uptime: i64,
    loads: [u64; 3],
    totalram: u64,
    freeram: u64,
    sharedram: u64,
    bufferram: u64,
    totalswap: u64,
    freeswap: u64,
    procs: u16,
    totalhigh: u64,
    freehigh: u64,
    mem_unit: u32,
    _padding: [u8; 20 - core::mem::size_of::<u32>() - core::mem::size_of::<u16>()],
}

pub fn syscall_getpid() -> SysResult<u64> {
    let pid = unsafe { crate::scheduler::pcb::current_pid() };
    Ok(pid as u64)
}

pub fn syscall_gettid() -> SysResult<u64> {
    let tid = unsafe { crate::scheduler::pcb::current_tid() };
    Ok(tid as u64)
}

pub fn syscall_clock_gettime(clk_id: u64, tp: UserPtr<TimeSpec>) -> SysResult<u64> {
    if !tp.is_aligned() {
        return Err(SysError::EFAULT);
    }
    let now_ns = unsafe { crate::timer::get_time_ns() };
    let ts = TimeSpec {
        tv_sec: (now_ns / 1_000_000_000) as i64,
        tv_nsec: (now_ns % 1_000_000_000) as i64,
    };
    unsafe { tp.write(ts)? };
    Ok(0)
}

pub fn syscall_futex(uaddr: UserPtr<i32>, futex_op: i32, val: i32, timeout: u64, uaddr2: UserPtr<i32>, val3: i32) -> SysResult<u64> {
    match futex_op {
        0 => {
            let current_val = unsafe { uaddr.read_volatile()? };
            if current_val != val {
                return Ok(-1i64 as u64);
            }
            unsafe { crate::sync::futex::wait(uaddr.as_ptr(), val).await };
            Ok(0)
        },
        1 => {
            unsafe { crate::sync::futex::wake(uaddr.as_ptr(), val) };
            Ok(0)
        },
        _ => Err(SysError::ENOSYS),
    }
}

pub fn syscall_stat(filename: UserPtr<u8>, statbuf: UserPtr<Stat>) -> SysResult<u64> {
    if !statbuf.is_aligned() {
        return Err(SysError::EFAULT);
    }

    let mut path_buf = [0u8; 256];
    let len = unsafe { filename.copy_to_slice(&mut path_buf)? };
    if len == 0 || path_buf[0] == 0 {
        return Err(SysError::ENOENT);
    }

    let stat = Stat {
        st_dev: 1,
        st_ino: 1,
        st_nlink: 1,
        st_mode: 0o100644,
        st_uid: 0,
        st_gid: 0,
        __pad0: 0,
        st_rdev: 0,
        st_size: 0,
        st_blksize: 512,
        st_blocks: 0,
        st_atim: TimeSpec { tv_sec: 0, tv_nsec: 0 },
        st_mtim: TimeSpec { tv_sec: 0, tv_nsec: 0 },
        st_ctim: TimeSpec { tv_sec: 0, tv_nsec: 0 },
        __unused: [0; 3],
    };

    unsafe { statbuf.write(stat)? };
    Ok(0)
}

pub fn syscall_open(filename: UserPtr<u8>, flags: i32, mode: u32) -> SysResult<u64> {
    let mut path_buf = [0u8; 256];
    let len = unsafe { filename.copy_to_slice(&mut path_buf)? };
    if len == 0 {
        return Err(SysError::ENOENT);
    }
    if path_buf[0] == b'/' && &path_buf[1..5] == b"dev" {
        if &path_buf[5..10] == b"/null" {
            return Ok(3);
        } else if &path_buf[5..9] == b"/zero" {
            return Ok(4);
        }
    }
    Err(SysError::ENOENT)
}

pub fn syscall_close(fd: u64) -> SysResult<u64> {
    if fd < 3 {
        return Ok(0);
    }
    if fd == 3 || fd == 4 {
        return Ok(0);
    }
    Err(SysError::EBADF)
}

pub fn syscall_dup(oldfd: u64) -> SysResult<u64> {
    if oldfd > 4 {
        return Err(SysError::EBADF);
    }
    Ok(oldfd)
}

pub fn syscall_pipe(fds: UserPtr<[i32; 2]>) -> SysResult<u64> {
    let pipe_fds = [5, 6];
    unsafe { fds.write(pipe_fds)? };
    Ok(0)
}

pub fn syscall_wait4(pid: i64, wstatus: UserPtr<i32>, options: u32, rusage: u64) -> SysResult<u64> {
    if pid == -1 {
        return Ok(!0);
    }
    if pid > 0 {
        let alive = unsafe { crate::scheduler::pcb::is_process_alive(pid as ProcessId) };
        if !alive {
            if !wstatus.is_null() {
                unsafe { wstatus.write(0)? };
            }
            return Ok(pid as u64);
        }
    }
    Ok(!0)
}

pub fn syscall_kill(pid: u64, sig: i32) -> SysResult<u64> {
    if sig == 0 {
        let exists = unsafe { crate::scheduler::pcb::is_process_alive(pid as ProcessId) };
        return if exists { Ok(0) } else { Err(SysError::ESRCH) };
    }
    if sig == 9 {
        unsafe { crate::scheduler::pcb::terminate_process(pid as ProcessId); }
        return Ok(0);
    }
    Err(SysError::EINVAL)
}

pub fn syscall_sigaction(sig: i32, act: UserPtr<u8>, oldact: UserPtr<u8>) -> SysResult<u64> {
    if sig < 1 || sig > 64 {
        return Err(SysError::EINVAL);
    }
    Ok(0)
}

pub fn syscall_rt_sigprocmask(how: i32, nset: UserPtr<u8>, oset: UserPtr<u8>, sigsetsize: usize) -> SysResult<u64> {
    if sigsetsize != 8 {
        return Err(SysError::EINVAL);
    }
    Ok(0)
}

pub fn syscall_poll(fds: UserPtr<PollFd>, nfds: u64, timeout: i32) -> SysResult<u64> {
    if nfds > 1024 {
        return Err(SysError::EINVAL);
    }
    let mut count = 0u64;
    for i in 0..nfds {
        let fd_ptr = unsafe { fds.offset(i as isize) };
        let mut fd_struct = unsafe { fd_ptr.read()? };
        if fd_struct.fd >= 0 && fd_struct.fd <= 6 {
            fd_struct.revents = fd_struct.events & 4;
            if fd_struct.revents != 0 {
                count += 1;
            }
            unsafe { fd_ptr.write(fd_struct)? };
        }
    }
    Ok(count)
}

pub fn syscall_ioctl(fd: u64, cmd: u64, arg: u64) -> SysResult<u64> {
    match fd {
        1 | 2 => match cmd {
            0x5401 => Ok(0),
            _ => Err(SysError::ENOTTY),
        },
        3 => match cmd {
            0x5401 => Ok(0),
            _ => Err(SysError::ENOTTY),
        },
        _ => Err(SysError::EBADF),
    }
}

pub fn syscall_clone(flags: u64, child_stack: u64, ptid: UserPtr<u64>, ctid: UserPtr<u64>, newtls: u64) -> SysResult<u64> {
    let new_pid = unsafe { crate::scheduler::pcb::create_clone(flags, child_stack, ptid.as_ptr(), ctid.as_ptr(), newtls) };
    if new_pid == 0 {
        Ok(0)
    } else {
        Ok(new_pid as u64)
    }
}

pub fn syscall_nanosleep(req: UserPtr<TimeSpec>, rem: UserPtr<TimeSpec>) -> SysResult<u64> {
    let timespec = unsafe { req.read()? };
    let ns = timespec.tv_sec as u64 * 1_000_000_000 + timespec.tv_nsec as u64;
    unsafe { crate::timer::sleep_ns(ns).await };
    if !rem.is_null() {
        unsafe { rem.write(TimeSpec { tv_sec: 0, tv_nsec: 0 })? };
    }
    Ok(0)
}

pub fn syscall_set_tid_address(tidptr: UserPtr<u64>) -> SysResult<u64> {
    unsafe { crate::scheduler::pcb::set_clear_child_tid(tidptr.as_ptr()) };
    let tid = unsafe { crate::scheduler::pcb::current_tid() };
    Ok(tid as u64)
}

pub fn syscall_arch_prctl(code: u64, addr: u64) -> SysResult<u64> {
    match code {
        0x1001 => {
            if addr & 0xFFF != 0 {
                return Err(SysError::EINVAL);
            }
            unsafe { crate::arch::cpu::set_fs_base(addr) };
            Ok(0)
        },
        0x1002 => {
            unsafe { crate::arch::cpu::set_gs_base(addr) };
            Ok(0)
        },
        _ => Err(SysError::EINVAL),
    }
}

pub fn syscall_getdents64(fd: u64, dirent: UserPtr<u8>, count: u64) -> SysResult<u64> {
    if fd != 3 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_mprotect(start: u64, len: u64, prot: u32) -> SysResult<u64> {
    if start & 0xFFF != 0 {
        return Err(SysError::EINVAL);
    }
    let end = start.saturating_add(len);
    unsafe { crate::memory::virtual::change_protection(start, end, prot)? };
    Ok(0)
}

pub fn syscall_munmap(addr: u64, length: u64) -> SysResult<u64> {
    if addr & 0xFFF != 0 {
        return Err(SysError::EINVAL);
    }
    unsafe { crate::memory::virtual::unmap_user_pages(addr, length)? };
    Ok(0)
}

pub fn syscall_sysinfo(info: UserPtr<SysInfo>) -> SysResult<u64> {
    let sys_info = SysInfo {
        uptime: unsafe { crate::timer::uptime_seconds() as i64 },
        loads: [0, 0, 0],
        totalram: 0x100000000,
        freeram: 0x80000000,
        sharedram: 0,
        bufferram: 0,
        totalswap: 0,
        freeswap: 0,
        procs: unsafe { crate::scheduler::pcb::get_process_count() as u16 },
        totalhigh: 0,
        freehigh: 0,
        mem_unit: 1,
        _padding: [0; 20 - core::mem::size_of::<u32>() - core::mem::size_of::<u16>()],
    };
    unsafe { info.write(sys_info)? };
    Ok(0)
}

pub fn syscall_getuid() -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_geteuid() -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_getgid() -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_getegid() -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_getppid() -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_getpgid(pid: u64) -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_setsid() -> SysResult<u64> {
    Ok(unsafe { crate::scheduler::pcb::current_pid() as u64 })
}

pub fn syscall_uname(name: UserPtr<UtsName>) -> SysResult<u64> {
    let uts = UtsName {
        sysname: [b'R'; 65],
        nodename: [0; 65],
        release: [b'1'; 65],
        version: [b'0'; 65],
        machine: [b'x'; 65],
        domainname: [0; 65],
    };
    unsafe { name.write(uts)? };
    Ok(0)
}

pub fn syscall_brk(addr: u64) -> SysResult<u64> {
    let new_brk = unsafe { crate::memory::virtual::set_brk(addr)? };
    Ok(new_brk)
}

pub fn syscall_mmap(addr: u64, len: u64, prot: u32, flags: u32, fd: i32, offset: u64) -> SysResult<u64> {
    if len == 0 {
        return Ok(addr);
    }
    if flags & 0x20 != 0 {
        return Ok(unsafe { crate::memory::virtual::alloc_huge_page(addr)? });
    }
    unsafe { crate::memory::virtual::map_user_pages(addr, len, prot, flags, fd, offset) }
}

pub fn syscall_writev(fd: u64, iov: UserPtr<IoVec>, iovcnt: u64) -> SysResult<u64> {
    if iovcnt == 0 || iovcnt > 1024 {
        return Err(SysError::EINVAL);
    }
    let mut total = 0;
    for i in 0..iovcnt {
        let io = unsafe { iov.offset(i as isize).read()? };
        if io.iov_len > 0 {
            let written = unsafe { crate::console::write_bytes(io.iov_base as *const u8, io.iov_len)? };
            total += written;
        }
    }
    Ok(total as u64)
}

pub fn syscall_readv(fd: u64, iov: UserPtr<IoVec>, iovcnt: u64) -> SysResult<u64> {
    if iovcnt == 0 || iovcnt > 1024 {
        return Err(SysError::EINVAL);
    }
    let mut total = 0;
    for i in 0..iovcnt {
        let mut io = unsafe { iov.offset(i as isize).read()? };
        if io.iov_len > 0 {
            let read = unsafe { crate::console::read_bytes(io.iov_base as *mut u8, io.iov_len)? };
            io.iov_len = read;
            total += read;
        }
    }
    Ok(total as u64)
}

pub fn syscall_pselect6(nfds: u64, readfds: UserPtr<u8>, writefds: UserPtr<u8>, exceptfds: UserPtr<u8>, timeout: UserPtr<TimeSpec>, sigmask: UserPtr<u8>) -> SysResult<u64> {
    if timeout.is_null() {
        return Ok(0);
    }
    let timespec = unsafe { timeout.read()? };
    let ns = timespec.tv_sec as u64 * 1_000_000_000 + timespec.tv_nsec as u64;
    unsafe { crate::timer::sleep_ns(ns).await };
    Ok(0)
}

pub fn syscall_ppoll(fds: UserPtr<PollFd>, nfds: u64, tmo_p: UserPtr<TimeSpec>, sigmask: UserPtr<u8>, sigsetsize: usize) -> SysResult<u64> {
    syscall_poll(fds, nfds, -1)
}

pub fn syscall_getrandom(buf: UserPtr<u8>, count: u64, flags: u32) -> SysResult<u64> {
    if count == 0 {
        return Ok(0);
    }
    let mut data = [0u8; 256];
    let c = core::cmp::min(count as usize, 256);
    for i in 0..c {
        data[i] = (i ^ (i << 1)) as u8;
    }
    unsafe { buf.copy_from_slice(&data[..c])? };
    Ok(c as u64)
}

pub fn syscall_getcpu(cpu: UserPtr<u32>, node: UserPtr<u32>, unused: u64) -> SysResult<u64> {
    if !cpu.is_null() {
        unsafe { cpu.write(0)? };
    }
    if !node.is_null() {
        unsafe { node.write(0)? };
    }
    Ok(0)
}

pub fn syscall_exit_group(status: i32) -> ! {
    unsafe { crate::scheduler::pcb::exit_current_process(status) };
}

pub fn syscall_epoll_create(size: i32) -> SysResult<u64> {
    if size <= 0 {
        return Err(SysError::EINVAL);
    }
    Ok(7)
}

pub fn syscall_epoll_ctl(epfd: u64, op: u32, fd: u64, event: UserPtr<u8>) -> SysResult<u64> {
    if epfd != 7 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_epoll_wait(epfd: u64, events: UserPtr<u8>, maxevents: i32, timeout: i32) -> SysResult<u64> {
    if epfd != 7 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_sendfile(out_fd: u64, in_fd: u64, offset: UserPtr<u64>, count: u64) -> SysResult<u64> {
    if out_fd == 1 && in_fd == 3 {
        Ok(0)
    } else {
        Err(SysError::EINVAL)
    }
}

pub fn syscall_fcntl(fd: u64, cmd: u32, arg: u64) -> SysResult<u64> {
    match cmd {
        0 => Ok(0),
        1 => Ok(0),
        3 => Ok(0),
        _ => Err(SysError::EINVAL),
    }
}

pub fn syscall_access(pathname: UserPtr<u8>, mode: u32) -> SysResult<u64> {
    let mut path_buf = [0u8; 256];
    let _len = unsafe { pathname.copy_to_slice(&mut path_buf)? };
    Ok(0)
}

pub fn syscall_chdir(path: UserPtr<u8>) -> SysResult<u64> {
    let mut path_buf = [0u8; 256];
    let _len = unsafe { path.copy_to_slice(&mut path_buf)? };
    Ok(0)
}

pub fn syscall_getcwd(buf: UserPtr<u8>, size: u64) -> SysResult<u64> {
    let cwd = b"/\0";
    if size < 2 {
        return Err(SysError::ERANGE);
    }
    unsafe { buf.copy_from_slice(cwd)? };
    Ok(2)
}

pub fn syscall_mkdir(pathname: UserPtr<u8>, mode: u32) -> SysResult<u64> {
    let mut path_buf = [0u8; 256];
    let _len = unsafe { pathname.copy_to_slice(&mut path_buf)? };
    Ok(0)
}

pub fn syscall_rmdir(pathname: UserPtr<u8>) -> SysResult<u64> {
    let mut path_buf = [0u8; 256];
    let _len = unsafe { pathname.copy_to_slice(&mut path_buf)? };
    Ok(0)
}

pub fn syscall_link(oldname: UserPtr<u8>, newname: UserPtr<u8>) -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_unlink(pathname: UserPtr<u8>) -> SysResult<u64> {
    let mut path_buf = [0u8; 256];
    let _len = unsafe { pathname.copy_to_slice(&mut path_buf)? };
    Ok(0)
}

pub fn syscall_rename(oldname: UserPtr<u8>, newname: UserPtr<u8>) -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_symlink(name1: UserPtr<u8>, name2: UserPtr<u8>) -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_readlink(path: UserPtr<u8>, buf: UserPtr<u8>, bufsiz: u64) -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_utime(path: UserPtr<u8>, times: UserPtr<u8>) -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_umask(mask: u32) -> SysResult<u64> {
    Ok(0o022)
}

pub fn syscall_chmod(pathname: UserPtr<u8>, mode: u32) -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_chown(pathname: UserPtr<u8>, owner: u32, group: u32) -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_lseek(fd: u64, offset: i64, whence: u32) -> SysResult<u64> {
    if fd > 6 {
        return Err(SysError::EBADF);
    }
    Ok(offset as u64)
}

pub fn syscall_truncate(path: UserPtr<u8>, length: u64) -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_ftruncate(fd: u64, length: u64) -> SysResult<u64> {
    if fd > 6 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_fstat(fd: u64, statbuf: UserPtr<Stat>) -> SysResult<u64> {
    if fd > 6 {
        return Err(SysError::EBADF);
    }
    let stat = Stat {
        st_dev: 1,
        st_ino: 1,
        st_nlink: 1,
        st_mode: 0o100644,
        st_uid: 0,
        st_gid: 0,
        __pad0: 0,
        st_rdev: 0,
        st_size: 0,
        st_blksize: 512,
        st_blocks: 0,
        st_atim: TimeSpec { tv_sec: 0, tv_nsec: 0 },
        st_mtim: TimeSpec { tv_sec: 0, tv_nsec: 0 },
        st_ctim: TimeSpec { tv_sec: 0, tv_nsec: 0 },
        __unused: [0; 3],
    };
    unsafe { statbuf.write(stat)? };
    Ok(0)
}

pub fn syscall_lstat(pathname: UserPtr<u8>, statbuf: UserPtr<Stat>) -> SysResult<u64> {
    syscall_stat(pathname, statbuf)
}

pub fn syscall_faccessat(dirfd: u64, pathname: UserPtr<u8>, mode: u32) -> SysResult<u64> {
    if dirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    syscall_access(pathname, mode)
}

pub fn syscall_fchmodat(dirfd: u64, pathname: UserPtr<u8>, mode: u32) -> SysResult<u64> {
    if dirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_fchownat(dirfd: u64, pathname: UserPtr<u8>, owner: u32, group: u32) -> SysResult<u64> {
    if dirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_mknod(pathname: UserPtr<u8>, mode: u32, dev: u64) -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_mknodat(dirfd: u64, pathname: UserPtr<u8>, mode: u32, dev: u64) -> SysResult<u64> {
    if dirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_futimesat(dirfd: u64, pathname: UserPtr<u8>, times: UserPtr<TimeSpec>) -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_newfstatat(dirfd: u64, pathname: UserPtr<u8>, statbuf: UserPtr<Stat>, flag: u32) -> SysResult<u64> {
    if dirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    syscall_stat(pathname, statbuf)
}

pub fn syscall_unlinkat(dirfd: u64, pathname: UserPtr<u8>, flags: u32) -> SysResult<u64> {
    if dirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_readahead(fd: u64, offset: u64, count: u64) -> SysResult<u64> {
    if fd > 6 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_sync_file_range(fd: u64, offset: u64, count: u64, flags: u32) -> SysResult<u64> {
    if fd > 6 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_fallocate(fd: u64, mode: u32, offset: u64, len: u64) -> SysResult<u64> {
    if fd > 6 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_timerfd_create(clockid: i32, flags: u32) -> SysResult<u64> {
    Ok(8)
}

pub fn syscall_eventfd(initval: u64, flags: u32) -> SysResult<u64> {
    Ok(9)
}

pub fn syscall_signalfd(fd: u64, mask: UserPtr<u8>, sizemask: u32) -> SysResult<u64> {
    Ok(10)
}

pub fn syscall_inotify_init() -> SysResult<u64> {
    Ok(11)
}

pub fn syscall_membarrier(cmd: u32, flags: u32, cpu_id: u32) -> SysResult<u64> {
    unsafe { core::arch::asm!("mfence") };
    Ok(0)
}

pub fn syscall_copy_file_range(fd_in: u64, off_in: UserPtr<u64>, fd_out: u64, off_out: UserPtr<u64>, length: u64, flags: u32) -> SysResult<u64> {
    if fd_in > 6 || fd_out > 6 {
        return Err(SysError::EBADF);
    }
    Ok(length)
}

pub fn syscall_preadv2(fd: u64, iov: UserPtr<IoVec>, iovcnt: u64, offset: u64, flags: u64) -> SysResult<u64> {
    if flags & 0x1000 != 0 {
        return Err(SysError::EOPNOTSUPP);
    }
    syscall_readv(fd, iov, iovcnt)
}

pub fn syscall_pwritev2(fd: u64, iov: UserPtr<IoVec>, iovcnt: u64, offset: u64, flags: u64) -> SysResult<u64> {
    if flags & 0x1000 != 0 {
        return Err(SysError::EOPNOTSUPP);
    }
    syscall_writev(fd, iov, iovcnt)
}

pub fn syscall_pkey_mprotect(addr: u64, len: u64, prot: u32, pkey: u32) -> SysResult<u64> {
    if pkey > 15 {
        return Err(SysError::EINVAL);
    }
    syscall_mprotect(addr, len, prot)
}

pub fn syscall_pkey_alloc(access_rights: u64, flags: u64) -> SysResult<u64> {
    if flags != 0 {
        return Err(SysError::EINVAL);
    }
    Ok(0)
}

pub fn syscall_pkey_free(pkey: u32) -> SysResult<u64> {
    if pkey > 15 {
        return Err(SysError::EINVAL);
    }
    Ok(0)
}

pub fn syscall_statx(dirfd: u64, pathname: UserPtr<u8>, flags: u32, mask: u32, buffer: UserPtr<u8>) -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_io_uring_setup(entries: u32, params: UserPtr<u8>) -> SysResult<u64> {
    Err(SysError::ENOSYS)
}

pub fn syscall_io_uring_enter(fd: u64, to_submit: u32, min_complete: u32, flags: u32, sig: u64) -> SysResult<u64> {
    Err(SysError::ENOSYS)
}

pub fn syscall_io_uring_register(fd: u64, opcode: u32, arg: u64, nr_args: u32) -> SysResult<u64> {
    Err(SysError::ENOSYS)
}

pub fn syscall_openat(dirfd: u64, pathname: UserPtr<u8>, flags: u32, mode: u32) -> SysResult<u64> {
    if dirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    syscall_open(pathname, flags as i32, mode)
}

pub fn syscall_mkdirat(dirfd: u64, pathname: UserPtr<u8>, mode: u32) -> SysResult<u64> {
    if dirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_mknodat(dirfd: u64, pathname: UserPtr<u8>, mode: u32, dev: u64) -> SysResult<u64> {
    if dirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_fchownat(dirfd: u64, pathname: UserPtr<u8>, owner: u32, group: u32) -> SysResult<u64> {
    if dirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_fstatat(dirfd: u64, pathname: UserPtr<u8>, statbuf: UserPtr<Stat>, flags: u32) -> SysResult<u64> {
    if dirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    syscall_newfstatat(dirfd, pathname, statbuf, flags)
}

pub fn syscall_unlinkat(dirfd: u64, pathname: UserPtr<u8>, flags: u32) -> SysResult<u64> {
    if dirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_readlinkat(dirfd: u64, pathname: UserPtr<u8>, buf: UserPtr<u8>, bufsiz: u64) -> SysResult<u64> {
    if dirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_symlinkat(target: UserPtr<u8>, newdirfd: u64, linkpath: UserPtr<u8>) -> SysResult<u64> {
    if newdirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_linkat(olddirfd: u64, oldpath: UserPtr<u8>, newdirfd: u64, newpath: UserPtr<u8>, flags: u32) -> SysResult<u64> {
    if olddirfd != -100i64 as u64 || newdirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_renameat(olddirfd: u64, oldpath: UserPtr<u8>, newdirfd: u64, newpath: UserPtr<u8>) -> SysResult<u64> {
    if olddirfd != -100i64 as u64 || newdirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_renameat2(olddirfd: u64, oldpath: UserPtr<u8>, newdirfd: u64, newpath: UserPtr<u8>, flags: u32) -> SysResult<u64> {
    if olddirfd != -100i64 as u64 || newdirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    if flags & !0x1F != 0 {
        return Err(SysError::EINVAL);
    }
    Ok(0)
}

pub fn syscall_futimesat(dirfd: u64, pathname: UserPtr<u8>, times: UserPtr<TimeSpec>) -> SysResult<u64> {
    if dirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_faccessat2(dirfd: u64, pathname: UserPtr<u8>, mode: u32, flags: u32) -> SysResult<u64> {
    if dirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    if flags != 0 {
        return Err(SysError::EINVAL);
    }
    Ok(0)
}

pub fn syscall_fchmodat2(dirfd: u64, pathname: UserPtr<u8>, mode: u32, flags: u32) -> SysResult<u64> {
    if dirfd != -100i64 as u64 {
        return Err(SysError::EBADF);
    }
    if flags != 0 {
        return Err(SysError::EINVAL);
    }
    Ok(0)
}

pub fn syscall_fstatat64(dirfd: u64, pathname: UserPtr<u8>, statbuf: UserPtr<Stat>, flags: u32) -> SysResult<u64> {
    syscall_fstatat(dirfd, pathname, statbuf, flags)
}

pub fn syscall_fcntl64(fd: u64, cmd: u32, arg: u64) -> SysResult<u64> {
    syscall_fcntl(fd, cmd, arg)
}

pub fn syscall_getdents(fd: u64, dirent: UserPtr<u8>, count: u64) -> SysResult<u64> {
    syscall_getdents64(fd, dirent, count)
}

pub fn syscall_fadvise64(fd: u64, offset: u64, len: u64, advice: u32) -> SysResult<u64> {
    if fd > 6 {
        return Err(SysError::EBADF);
    }
    Ok(0)
}

pub fn syscall_fadvise64_64(fd: u64, offset: u64, len: u64, advice: u32) -> SysResult<u64> {
    syscall_fadvise64(fd, offset, len, advice)
}

pub fn syscall_fallocate64(fd: u64, mode: u32, offset: u64, len: u64) -> SysResult<u64> {
    syscall_fallocate(fd, mode, offset, len)
}

pub fn syscall_ftruncate64(fd: u64, length: u64) -> SysResult<u64> {
    syscall_ftruncate(fd, length)
}

pub fn syscall_truncate64(path: UserPtr<u8>, length: u64) -> SysResult<u64> {
    Ok(0)
}

pub fn syscall_lseek64(fd: u64, offset: i64, whence: u32) -> SysResult<u64> {
    syscall_lseek(fd, offset, whence)
}

pub fn syscall_sendfile64(out_fd: u64, in_fd: u64, offset: UserPtr<u64>, count: u64) -> SysResult<u64> {
    syscall_sendfile(out_fd, in_fd, offset, count)
}

pub fn syscall_sync_file_range2(fd: u64, flags: u32, offset: u64, count: u64) -> SysResult<u64> {
    syscall_sync_file_range(fd, offset, count, flags)
}

pub fn syscall_dup2(oldfd: u64, newfd: u64) -> SysResult<u64> {
    if oldfd > 6 || newfd > 6 {
        return Err(SysError::EBADF);
    }
    Ok(newfd)
}

pub fn syscall_dup3(oldfd: u64, newfd: u64, flags: u32) -> SysResult<u64> {
    if oldfd > 6 || newfd > 6 {
        return Err(SysError::EBADF);
    }
    if flags & !0x1000 != 0 {
        return Err(SysError::EINVAL);
    }
    Ok(newfd)
}

pub fn syscall_pipe2(fds: UserPtr<[i32; 2]>, flags: u32) -> SysResult<u64> {
    if flags & !0x400 != 0 {
        return Err(SysError::EINVAL);
    }
    syscall_pipe(fds)
}

pub fn syscall_socket(domain: u64, type_: u64, protocol: u64) -> SysResult<u64> {
    Err(SysError::EAFNOSUPPORT)
}

pub fn syscall_bind(sockfd: u64, addr: UserPtr<u8>, addrlen: u32) -> SysResult<u64> {
    Err(SysError::EBADF)
}

pub fn syscall_connect(sockfd: u64, addr: UserPtr<u8>, addrlen: u32) -> SysResult<u64> {
    Err(SysError::EBADF)
}

pub fn syscall_listen(sockfd: u64, backlog: u32) -> SysResult<u64> {
    Err(SysError::EBADF)
}

pub fn syscall_accept(sockfd: u64, addr: UserPtr<u8>, addrlen: UserPtr<u32>) -> SysResult<u64> {
    Err(SysError::EBADF)
}

pub fn syscall_getsockname(sockfd: u64, addr: UserPtr<u8>, addrlen: UserPtr<u32>) -> SysResult<u64> {
    Err(SysError::EBADF)
}

pub fn syscall_getpeername(sockfd: u64, addr: UserPtr<u8>, addrlen: UserPtr<u32>) -> SysResult<u64> {
    Err(SysError::EBADF)
}

pub fn syscall_socketpair(domain: u64, type_: u64, protocol: u64, sv: UserPtr<[i32; 2]>) -> SysResult<u64> {
    Err(SysError::EAFNOSUPPORT)
}

pub fn syscall_send(sockfd: u64, buf: UserPtr<u8>, len: u64, flags: u64) -> SysResult<u64> {
    Err(SysError::EBADF)
}

pub fn syscall_sendto(sockfd: u64, buf: UserPtr<u8>, len: u64, flags: u64, dest_addr: UserPtr<u8>, addrlen: u32) -> SysResult<u64> {
    Err(SysError::EBADF)
}

pub fn syscall_recv(sockfd: u64, buf: UserPtr<u8>, len: u64, flags: u64) -> SysResult<u64> {
    Err(SysError::EBADF)
}

pub fn syscall_recvfrom(sockfd: u64, buf: UserPtr<u8>, len: u64, flags: u64, src_addr: UserPtr<u8>, addrlen: UserPtr<u32>) -> SysResult<u64> {
    Err(SysError::EBADF)
}

pub fn syscall_shutdown(sockfd: u64, how: u64) -> SysResult<u64> {
    Err(SysError::EBADF)
}

pub fn syscall_setsockopt(sockfd: u64, level: u64, optname: u64, optval: UserPtr<u8>, optlen: u32) -> SysResult<u64> {
    Err(SysError::EBADF)
}

pub fn syscall_getsockopt(sockfd: u64, level: u64, optname: u64, optval: UserPtr<u8>, optlen: UserPtr<u32>) -> SysResult<u64> {
    Err(SysError::EBADF)
}

pub fn syscall_sendmsg(sockfd: u64, msg: UserPtr<u8>, flags: u64) -> SysResult<u64> {
    Err(SysError::EBADF)
}

pub fn syscall_recvmsg(sockfd: u64, msg: UserPtr<u8>, flags: u64) -> SysResult<u64> {
    Err(SysError::EBADF)
}

