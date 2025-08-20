use crate::task::context::TaskContext;

enum TaskStatus {
    Ready,
    Running,
    Terminated,
}
struct TaskControlBlock{
    task_id: usize,
    status: TaskStatus,
    context: TaskContext
}