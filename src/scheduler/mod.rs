pub mod pcb;
pub mod scheduler;

pub use pcb::ProcessControlBlock;
pub use scheduler::{schedule, yield_now, init_scheduler};

pub fn get_current_pid() -> u32 {
    unsafe { CURRENT_PID }
}

static mut CURRENT_PID: u32 = 0;

pub fn set_current_pid(pid: u32) {
    unsafe { CURRENT_PID = pid; }
}

pub const MAX_TASKS: usize = 64;

pub struct TaskManager;

impl TaskManager {
    pub fn new() -> Self {
        TaskManager
    }
}

pub fn spawn_task(_func: fn()) {
    
}

pub fn task_exit(_code: i32) {

}

