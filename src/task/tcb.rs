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
    pub entry_point: (usize, usize),
    pub stack_base: NonNull<u8>,
    pub page_count: usize,
    pub priority: u8,
    pub status: TaskStatus,
    pub context: TaskContext,
}

unsafe impl Send for TaskContext {}
