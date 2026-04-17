use core::arch::asm;
use crate::scheduler::pcb::{ProcessControlBlock, TaskState};
use crate::arch::cpu;

const MAX_TASKS: usize = 1024;
static mut RUNQUEUE: [Option<&'static mut ProcessControlBlock>; MAX_TASKS] = [None; MAX_TASKS];
static mut CURRENT_TASK: Option<&'static mut ProcessControlBlock> = None;
static mut TASK_COUNT: usize = 0;
static mut SCHED_LOCK: bool = false;
static mut NEXT_TASK_ID: u64 = 1;

pub fn schedule() {
    unsafe {
        if SCHED_LOCK { return }
        SCHED_LOCK = true;
        
        let current = CURRENT_TASK.as_mut().unwrap();
        if current.state == TaskState::Running {
            current.state = TaskState::Ready;
        }

        let next_idx = find_next_task();
        if next_idx == -1 {
            SCHED_LOCK = false;
            cpu::halt();
            return;
        }

        let next = RUNQUEUE[next_idx as usize].as_mut().unwrap();
        if next.id != current.id {
            next.state = TaskState::Running;
            CURRENT_TASK = Some(&mut *next);
            context_switch(current, next);
        }
        SCHED_LOCK = false;
    }
}

unsafe fn find_next_task() -> i32 {
    let current = CURRENT_TASK.as_ref().unwrap().id;
    let start = (current % MAX_TASKS) + 1;
    
    for i in 0..MAX_TASKS {
        let idx = (start + i) % MAX_TASKS;
        if let Some(task) = &mut RUNQUEUE[idx] {
            if task.state == TaskState::Ready {
                return idx as i32;
            }
        }
    }
    -1
}

fn context_switch(prev: &mut ProcessControlBlock, next: &mut ProcessControlBlock) {
    unsafe {
        asm!(
            "push rbp",
            "push rbx",
            "push rcx",
            "push rdx",
            "push rsi",
            "push rdi",
            "push r8",
            "push r9",
            "push r10",
            "push r11",
            "push r12",
            "push r13",
            "push r14",
            "push r15",
            out("rax") _,
            out("rdx") _,
        );
        
        prev.regs = core::ptr::read_volatile(&core::arch::asm!("mov {}, rsp", out(reg) _) as u64);
        
        core::arch::asm!("mov rsp, {}", in(reg) next.regs);
        
        asm!(
            "pop r15",
            "pop r14",
            "pop r13",
            "pop r12",
            "pop r11",
            "pop r10",
            "pop r9",
            "pop r8",
            "pop rdi",
            "pop rsi",
            "pop rdx",
            "pop rcx",
            "pop rbx",
            "pop rbp",
            out("rax") _,
        );
    }
}

pub fn add_task(task: &'static mut ProcessControlBlock) {
    unsafe {
        if TASK_COUNT >= MAX_TASKS { return }
        for i in 0..MAX_TASKS {
            if RUNQUEUE[i].is_none() {
                RUNQUEUE[i] = Some(task);
                TASK_COUNT += 1;
                break;
            }
        }
    }
}

pub fn tick() {
    unsafe {
        if let Some(ref mut current) = CURRENT_TASK {
            current.tick_count += 1;
            if current.tick_count >= current.time_slice {
                current.tick_count = 0;
                schedule();
            }
        } else {
            schedule();
        }
    }
}

pub fn yield_cpu() {
    schedule();
}

pub fn idle_loop() -> ! {
    loop {
        unsafe {
            if TASK_COUNT > 1 {
                schedule();
            } else {
                cpu::halt();
            }
        }
    }
}

pub fn init_scheduler(first_task: &'static mut ProcessControlBlock) {
    unsafe {
        CURRENT_TASK = Some(first_task);
        first_task.state = TaskState::Running;
        add_task(first_task);
    }
}

pub fn remove_current_task() {
    unsafe {
        if let Some(task) = CURRENT_TASK.take() {
            for i in 0..MAX_TASKS {
                if RUNQUEUE[i].as_ref().map_or(false, |t| t.id == task.id) {
                    RUNQUEUE[i] = None;
                    TASK_COUNT -= 1;
                    break;
                }
            }
        }
    }
}

pub fn get_current_task_id() -> u64 {
    unsafe {
        CURRENT_TASK.as_ref().map_or(0, |task| task.id)
    }
}

pub fn set_task_ready(task_id: u64) {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &mut RUNQUEUE[i] {
                if task.id == task_id {
                    task.state = TaskState::Ready;
                    break;
                }
            }
        }
    }
}

pub fn preempt_disable() {
    unsafe {
        SCHED_LOCK = true;
    }
}

pub fn preempt_enable() {
    unsafe {
        SCHED_LOCK = false;
    }
}

pub fn is_preemptible() -> bool {
    unsafe { !SCHED_LOCK }
}

pub fn create_task(entry: u64, stack_top: u64) -> Option<u64> {
    unsafe {
        if TASK_COUNT >= MAX_TASKS { return None }
        let task_id = NEXT_TASK_ID;
        NEXT_TASK_ID += 1;
        
        for i in 0..MAX_TASKS {
            if RUNQUEUE[i].is_none() {
                let pcb = ProcessControlBlock::new(task_id, entry, stack_top);
                RUNQUEUE[i] = Some(pcb);
                TASK_COUNT += 1;
                return Some(task_id);
            }
        }
        None
    }
}

pub fn get_task_state(task_id: u64) -> Option<TaskState> {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &RUNQUEUE[i] {
                if task.id == task_id {
                    return Some(task.state);
                }
            }
        }
        None
    }
}

pub fn set_task_blocked(task_id: u64) {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &mut RUNQUEUE[i] {
                if task.id == task_id {
                    task.state = TaskState::Blocked;
                    break;
                }
            }
        }
    }
}

pub fn get_runqueue_size() -> usize {
    unsafe { TASK_COUNT }
}

pub fn reschedule_if_needed() {
    unsafe {
        if SCHED_LOCK { return }
        if let Some(ref current) = CURRENT_TASK {
            if current.state != TaskState::Running && current.state != TaskState::Idle {
                schedule();
            }
        }
    }
}

pub fn get_next_task_id() -> u64 {
    unsafe { NEXT_TASK_ID }
}

pub fn reset_scheduler() {
    unsafe {
        for i in 0..MAX_TASKS {
            RUNQUEUE[i] = None;
        }
        CURRENT_TASK = None;
        TASK_COUNT = 0;
        SCHED_LOCK = false;
        NEXT_TASK_ID = 1;
    }
}

pub fn get_current_task_mut() -> Option<&'static mut ProcessControlBlock> {
    unsafe { CURRENT_TASK.as_mut() }
}

pub fn get_current_task() -> Option<&'static ProcessControlBlock> {
    unsafe { CURRENT_TASK.as_ref() }
}

pub fn force_context_switch() {
    unsafe {
        if let Some(current) = &mut CURRENT_TASK {
            if current.state == TaskState::Running {
                current.state = TaskState::Ready;
            }
        }
        schedule();
    }
}

pub fn get_task_by_id(task_id: u64) -> Option<&'static mut ProcessControlBlock> {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &mut RUNQUEUE[i] {
                if task.id == task_id {
                    return Some(task);
                }
            }
        }
        None
    }
}

pub fn update_current_task_priority(priority: u8) {
    unsafe {
        if let Some(current) = &mut CURRENT_TASK {
            current.priority = priority;
        }
    }
}

pub fn get_highest_priority_ready_task() -> Option<u64> {
    unsafe {
        let mut highest_prio = 255;
        let mut target_task_id = 0;
        
        for i in 0..MAX_TASKS {
            if let Some(task) = &RUNQUEUE[i] {
                if task.state == TaskState::Ready && task.priority < highest_prio {
                    highest_prio = task.priority;
                    target_task_id = task.id;
                }
            }
        }
        
        if target_task_id != 0 { Some(target_task_id) } else { None }
    }
}

pub fn schedule_with_policy() {
    if let Some(next_id) = get_highest_priority_ready_task() {
        unsafe {
            if let Some(current) = &CURRENT_TASK {
                if current.id != next_id {
                    for i in 0..MAX_TASKS {
                        if let Some(task) = &mut RUNQUEUE[i] {
                            if task.id == next_id {
                                task.state = TaskState::Running;
                                CURRENT_TASK = Some(&mut *task);
                                context_switch(current, task);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn increment_current_task_time() {
    unsafe {
        if let Some(current) = &mut CURRENT_TASK {
            current.execution_time += 1;
        }
    }
}

pub fn get_current_execution_time() -> u64 {
    unsafe {
        CURRENT_TASK.as_ref().map_or(0, |task| task.execution_time)
    }
}

pub fn get_total_tasks() -> usize {
    unsafe { TASK_COUNT }
}

pub fn get_idle_task() -> &'static mut ProcessControlBlock {
    unsafe {
        static mut IDLE_PCB: Option<ProcessControlBlock> = None;
        if IDLE_PCB.is_none() {
            IDLE_PCB = Some(ProcessControlBlock::new(0, 0, 0));
            IDLE_PCB.as_mut().unwrap().state = TaskState::Idle;
        }
        IDLE_PCB.as_mut().unwrap()
    }
}

pub fn set_current_task_state(state: TaskState) {
    unsafe {
        if let Some(current) = &mut CURRENT_TASK {
            current.state = state;
        }
    }
}

pub fn get_current_task_state() -> TaskState {
    unsafe {
        CURRENT_TASK.as_ref().map_or(TaskState::Invalid, |task| task.state)
    }
}

pub fn check_preemption_conditions() -> bool {
    unsafe {
        if let Some(current) = &CURRENT_TASK {
            current.tick_count >= current.time_slice || current.state != TaskState::Running
        } else {
            true
        }
    }
}

pub fn set_task_quantum(task_id: u64, quantum: u32) {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &mut RUNQUEUE[i] {
                if task.id == task_id {
                    task.time_slice = quantum;
                    break;
                }
            }
        }
    }
}

pub fn get_task_quantum(task_id: u64) -> Option<u32> {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &RUNQUEUE[i] {
                if task.id == task_id {
                    return Some(task.time_slice);
                }
            }
        }
        None
    }
}

pub fn handle_timer_interrupt() {
    tick();
}

pub fn wake_up_tasks() {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &mut RUNQUEUE[i] {
                if task.state == TaskState::Sleeping && task.wake_time <= get_current_execution_time() {
                    task.state = TaskState::Ready;
                }
            }
        }
    }
}

pub fn sleep_current_task(duration: u64) {
    unsafe {
        if let Some(current) = &mut CURRENT_TASK {
            current.wake_time = get_current_execution_time() + duration;
            current.state = TaskState::Sleeping;
            schedule();
        }
    }
}

pub fn get_current_remaining_quantum() -> u32 {
    unsafe {
        CURRENT_TASK.as_ref().map_or(0, |task| {
            if task.time_slice > task.tick_count {
                task.time_slice - task.tick_count
            } else {
                0
            }
        })
    }
}

pub fn enable_round_robin() {
    // RR уже используется по умолчанию
}

pub fn disable_scheduler() {
    unsafe {
        SCHED_LOCK = true;
    }
}

pub fn enable_scheduler() {
    unsafe {
        SCHED_LOCK = false;
    }
}

pub fn is_scheduler_enabled() -> bool {
    unsafe { !SCHED_LOCK }
}

pub fn get_scheduler_lock_status() -> bool {
    unsafe { SCHED_LOCK }
}

pub fn get_task_priority(task_id: u64) -> Option<u8> {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &RUNQUEUE[i] {
                if task.id == task_id {
                    return Some(task.priority);
                }
            }
        }
        None
    }
}

pub fn set_global_time_slice(time_slice: u32) {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &mut RUNQUEUE[i] {
                task.time_slice = time_slice;
            }
        }
    }
}

pub fn get_current_task_priority() -> u8 {
    unsafe {
        CURRENT_TASK.as_ref().map_or(128, |task| task.priority)
    }
}

pub fn update_task_statistics(task_id: u64) {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &mut RUNQUEUE[i] {
                if task.id == task_id {
                    task.last_schedule_tick = get_current_execution_time();
                    break;
                }
            }
        }
    }
}

pub fn get_last_schedule_tick(task_id: u64) -> Option<u64> {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &RUNQUEUE[i] {
                if task.id == task_id {
                    return Some(task.last_schedule_tick);
                }
            }
        }
        None
    }
}

pub fn force_schedule() {
    unsafe {
        SCHED_LOCK = false;
        schedule();
    }
}

pub fn get_task_count() -> usize {
    unsafe { TASK_COUNT }
}

pub fn is_task_running(task_id: u64) -> bool {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &RUNQUEUE[i] {
                if task.id == task_id && task.state == TaskState::Running {
                    return true;
                }
            }
        }
        false
    }
}

pub fn is_any_task_ready() -> bool {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &RUNQUEUE[i] {
                if task.state == TaskState::Ready {
                    return true;
                }
            }
        }
        false
    }
}

pub fn get_next_scheduled_task_id() -> Option<u64> {
    if let Some(id) = get_highest_priority_ready_task() {
        Some(id)
    } else {
        unsafe {
            for i in 0..MAX_TASKS {
                if let Some(task) = &RUNQUEUE[i] {
                    if task.state == TaskState::Ready {
                        return Some(task.id);
                    }
                }
            }
        }
        None
    }
}

pub fn cleanup_zombie_tasks() {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &RUNQUEUE[i] {
                if task.state == TaskState::Zombie {
                    RUNQUEUE[i] = None;
                    TASK_COUNT -= 1;
                }
            }
        }
    }
}

pub fn set_task_exit_code(task_id: u64, exit_code: i32) {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &mut RUNQUEUE[i] {
                if task.id == task_id {
                    task.exit_code = exit_code;
                    break;
                }
            }
        }
    }
}

pub fn get_task_exit_code(task_id: u64) -> Option<i32> {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &RUNQUEUE[i] {
                if task.id == task_id {
                    return Some(task.exit_code);
                }
            }
        }
        None
    }
}

pub fn mark_task_as_zombie(task_id: u64) {
    unsafe {
        for i in 0..MAX_TASKS {
            if let Some(task) = &mut RUNQUEUE[i] {
                if task.id == task_id {
                    task.state = TaskState::Zombie;
                    break;
                }
            }
        }
    }
}

pub fn is_scheduler_locked() -> bool {
    unsafe { SCHED_LOCK }
}

pub fn wait_for_task_exit(task_id: u64) -> i32 {
    loop {
        if let Some(code) = get_task_exit_code(task_id) {
            return
