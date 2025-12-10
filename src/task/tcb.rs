use core::{alloc::Layout, ptr::NonNull};

use crate::task::context::TaskContext;

#[derive(PartialEq, Debug)]
pub enum TaskStatus {
    Ready,
    Running,
    Blocked,
    Terminated,
}

#[derive(Debug)]
pub struct TaskControlBlock {
    pub task_id: usize,
    pub entry_point: extern "C" fn(),
    pub stack_base: NonNull<u8>,
    pub layout: Layout,
    pub priority: u8,
    pub status: TaskStatus,
    pub context: TaskContext,
}

unsafe impl Send for TaskContext {}
