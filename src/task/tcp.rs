use crate::task::context::TaskContext;

enum TaskStatus {
    Ready,
    Running,
    Blocked,
    Terminated,
}
pub struct TaskControlBlock {
    task_id: usize,
    stack_base: usize,
    stack_size: usize,
    entry_point: usize,
    priority: u8,
    status: TaskStatus,
    context: TaskContext,
}
