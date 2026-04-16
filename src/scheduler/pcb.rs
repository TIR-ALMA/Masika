use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use core::cell::UnsafeCell;
use crate::arch::cpu::{save_fpu_state, restore_fpu_state};

#[repr(C)]
pub struct RegisterContext {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub rflags: u64,
    pub fs_base: u64,
    pub gs_base: u64,
}

#[repr(C)]
pub struct FpuState {
    pub data: [u8; 512],
}

impl FpuState {
    pub fn new() -> Self {
        Self { data: [0; 512] }
    }

    pub fn save(&mut self) {
        unsafe { save_fpu_state(self.data.as_mut_ptr()); }
    }

    pub fn restore(&self) {
        unsafe { restore_fpu_state(self.data.as_ptr()); }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum ProcessState {
    Running,
    Ready,
    Blocked,
    Zombie,
    Sleeping,
    Idle,
}

#[derive(Clone, Copy)]
pub struct ProcessId(u64);

static PID_COUNTER: AtomicU64 = AtomicU64::new(1);

impl ProcessId {
    pub fn new() -> Self {
        Self(PID_COUNTER.fetch_add(1, Ordering::SeqCst))
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

pub struct ProcessControlBlock {
    pub pid: ProcessId,
    pub parent_pid: Option<ProcessId>,
    pub state: ProcessState,
    pub registers: RegisterContext,
    pub kernel_stack_top: u64,
    pub user_stack_top: u64,
    pub fpu_state: FpuState,
    pub priority: u8,
    pub ticks_remaining: u32,
    pub exit_code: Option<u8>,
    pub wait_queue: *mut usize,
    pub signal_mask: u64,
    pub pending_signals: u64,
    pub page_table_root: u64,
    pub cwd_inode: u64,
    pub umask: u16,
    pub uid: u32,
    pub euid: u32,
    pub gid: u32,
    pub egid: u32,
    pub start_time: u64,
    pub cpu_time: u64,
    pub thread_id: u64,
    pub stack_bottom: u64,
    pub tls_area: u64,
    pub personality: u32,
    pub flags: u32,
    pub alarm_timer: u64,
    pub vfork_parent: Option<ProcessId>,
    pub ptrace_tracer: Option<ProcessId>,
    pub ptrace_flags: u32,
    pub seccomp_mode: u8,
    pub robust_list_head: u64,
    pub clear_tid_addr: u64,
    pub restart_block: u64,
    pub itimers: [u64; 3],
    pub rusage: [u64; 16],
    pub comm: [u8; 16],
    pub pending_children: usize,
    pub num_threads: u32,
    pub group_leader: ProcessId,
    pub session_id: ProcessId,
    pub process_group: ProcessId,
    pub exit_signal: i32,
    pub exit_state: u32,
    pub jobctl: u32,
    pub task_works: u64,
    pub bpf_ctx: u64,
    pub last_switch_time: u64,
    pub sched_time_slice: u32,
    pub io_wait_count: u32,
    pub preempt_count: u32,
    pub static_prio: u8,
    pub normal_prio: u8,
    pub deadline: u64,
    pub virtual_runtime: u64,
    pub sleep_start: u64,
    pub in_kernel: bool,
    pub is_idle: bool,
    pub needs_resched: AtomicBool,
    pub in_syscall: bool,
    pub in_irq: bool,
    pub fpu_owner: bool,
    pub mm_lock: UnsafeCell<u32>,
    pub files_lock: UnsafeCell<u32>,
    pub fs_lock: UnsafeCell<u32>,
    pub signal_lock: UnsafeCell<u32>,
    pub cred_lock: UnsafeCell<u32>,
    pub ptrace_lock: UnsafeCell<u32>,
    pub cleanup_state: u8,
    pub exit_link: Option<ProcessId>,
    pub numa_policy: u8,
    pub cpus_allowed: [u64; 4],
    pub mems_allowed: [u64; 4],
    pub node_vm_start: u64,
    pub node_vm_end: u64,
    pub last_cpu: u32,
    pub wake_cpu: u32,
    pub on_rq: bool,
    pub runnable: bool,
    pub running: bool,
    pub policy: u8,
    pub rt_priority: u8,
    pub slice: u32,
    pub nr_switches: u64,
    pub nr_voluntary_switches: u64,
    pub nr_involuntary_switches: u64,
    pub last_arrival: u64,
    pub avg_arrival: u64,
    pub load_weight: u32,
    pub exec_start: u64,
    pub stats: [u64; 8],
}

impl ProcessControlBlock {
    pub fn new(kernel_stack: u64, user_stack: u64, entry_point: u64) -> Self {
        let now = unsafe { core::arch::x86_64::_rdtsc() };
        Self {
            pid: ProcessId::new(),
            parent_pid: None,
            state: ProcessState::Ready,
            registers: RegisterContext {
                rax: 0,
                rbx: 0,
                rcx: 0,
                rdx: 0,
                rsi: 0,
                rdi: 0,
                rbp: 0,
                rsp: user_stack,
                r8: 0,
                r9: 0,
                r10: 0,
                r11: 0,
                r12: 0,
                r13: 0,
                r14: 0,
                r15: 0,
                rip: entry_point,
                rflags: 0x202,
                fs_base: 0,
                gs_base: 0,
            },
            kernel_stack_top: kernel_stack,
            user_stack_top: user_stack,
            fpu_state: FpuState::new(),
            priority: 10,
            ticks_remaining: 10,
            exit_code: None,
            wait_queue: core::ptr::null_mut(),
            signal_mask: 0,
            pending_signals: 0,
            page_table_root: 0,
            cwd_inode: 0,
            umask: 0o022,
            uid: 0,
            euid: 0,
            gid: 0,
            egid: 0,
            start_time: now,
            cpu_time: 0,
            thread_id: 0,
            stack_bottom: 0,
            tls_area: 0,
            personality: 0,
            flags: 0,
            alarm_timer: 0,
            vfork_parent: None,
            ptrace_tracer: None,
            ptrace_flags: 0,
            seccomp_mode: 0,
            robust_list_head: 0,
            clear_tid_addr: 0,
            restart_block: 0,
            itimers: [0; 3],
            rusage: [0; 16],
            comm: [0; 16],
            pending_children: 0,
            num_threads: 1,
            group_leader: ProcessId::new(),
            session_id: ProcessId::new(),
            process_group: ProcessId::new(),
            exit_signal: 17,
            exit_state: 0,
            jobctl: 0,
            task_works: 0,
            bpf_ctx: 0,
            last_switch_time: 0,
            sched_time_slice: 1000000,
            io_wait_count: 0,
            preempt_count: 0,
            static_prio: 120,
            normal_prio: 120,
            deadline: 0,
            virtual_runtime: 0,
            sleep_start: 0,
            in_kernel: false,
            is_idle: false,
            needs_resched: AtomicBool::new(false),
            in_syscall: false,
            in_irq: false,
            fpu_owner: false,
            mm_lock: UnsafeCell::new(0),
            files_lock: UnsafeCell::new(0),
            fs_lock: UnsafeCell::new(0),
            signal_lock: UnsafeCell::new(0),
            cred_lock: UnsafeCell::new(0),
            ptrace_lock: UnsafeCell::new(0),
            cleanup_state: 0,
            exit_link: None,
            numa_policy: 0,
            cpus_allowed: [u64::MAX, 0, 0, 0],
            mems_allowed: [u64::MAX, 0, 0, 0],
            node_vm_start: 0,
            node_vm_end: 0,
            last_cpu: 0,
            wake_cpu: 0,
            on_rq: false,
            runnable: true,
            running: false,
            policy: 0,
            rt_priority: 0,
            slice: 1000000,
            nr_switches: 0,
            nr_voluntary_switches: 0,
            nr_involuntary_switches: 0,
            last_arrival: 0,
            avg_arrival: 0,
            load_weight: 1024,
            exec_start: 0,
            stats: [0; 8],
        }
    }

    pub fn set_ready(&mut self) {
        self.state = ProcessState::Ready;
        self.ticks_remaining = self.priority as u32;
        self.runnable = true;
        self.on_rq = true;
    }

    pub fn set_running(&mut self) {
        self.state = ProcessState::Running;
        self.running = true;
        self.runnable = false;
        self.on_rq = false;
    }

    pub fn set_blocked(&mut self) {
        self.state = ProcessState::Blocked;
        self.runnable = false;
        self.on_rq = false;
    }

    pub fn set_zombie(&mut self, code: u8) {
        self.state = ProcessState::Zombie;
        self.exit_code = Some(code);
        self.runnable = false;
        self.on_rq = false;
    }

    pub fn save_fpu(&mut self) {
        self.fpu_state.save();
        self.fpu_owner = false;
    }

    pub fn restore_fpu(&self) {
        self.fpu_state.restore();
    }

    pub fn is_runnable(&self) -> bool {
        matches!(self.state, ProcessState::Ready | ProcessState::Running) && self.runnable
    }

    pub fn has_pending_signals(&self) -> bool {
        self.pending_signals != 0 && !self.in_kernel
    }

    pub fn clear_signal(&mut self, sig: u8) {
        self.pending_signals &= !(1 << sig);
    }

    pub fn add_signal(&mut self, sig: u8) {
        if (self.signal_mask & (1 << sig)) == 0 {
            self.pending_signals |= 1 << sig;
        }
    }

    pub fn update_cpu_time(&mut self) {
        let now = unsafe { core::arch::x86_64::_rdtsc() };
        self.cpu_time += now - self.last_switch_time;
        self.last_switch_time = now;
    }

    pub fn get_comm_str(&self) -> &str {
        let len = self.comm.iter().position(|&c| c == 0).unwrap_or(16);
        core::str::from_utf8(&self.comm[..len]).unwrap_or("")
    }

    pub fn set_comm(&mut self, name: &str) {
        let len = core::cmp::min(name.len(), 15);
        self.comm[..len].copy_from_slice(&name.as_bytes()[..len]);
        self.comm[len] = 0;
    }
}

