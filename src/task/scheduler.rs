use crate::{
    mm::HEAP_ALLOCATOR,
    polling_println, println,
    task::{
        SCHEDULER,
        context::TaskContext,
        switch::__switch_to,
        tcb::{TaskControlBlock, TaskStatus},
    },
    trap::trap_handler,
};
use alloc::{
    collections::{binary_heap::BinaryHeap, vec_deque::VecDeque},
    vec::Vec,
};
use core::{
    alloc::{GlobalAlloc, Layout, LayoutError},
    cmp::Reverse,
    ptr::NonNull,
};

#[derive(Debug)]
pub enum SchedulerError {
    SchedulerLayoutError(LayoutError),
    MemoryAllocationError,
}

impl From<LayoutError> for SchedulerError {
    fn from(value: LayoutError) -> Self {
        Self::SchedulerLayoutError(value)
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SleepEntry {
    wake_tick: usize,
    task_id: usize,
}

type TaskId = usize;
pub struct Scheduler {
    current_task_id: Option<TaskId>,
    ready_queue: VecDeque<TaskId>,
    task_list: Vec<Option<TaskControlBlock>>,
    zombie_queue: Vec<TaskId>,
    blocked_queue: BinaryHeap<Reverse<SleepEntry>>,
}
unsafe impl Send for Scheduler {}
impl Scheduler {
    pub const fn new() -> Scheduler {
        Scheduler {
            current_task_id: None,
            ready_queue: VecDeque::new(),
            task_list: Vec::new(),
            zombie_queue: Vec::new(),
            blocked_queue: BinaryHeap::new(),
        }
    }
    pub fn init() -> Result<(), SchedulerError> {
        let mut scheduler = SCHEDULER.lock();
        scheduler.spawn(idle_task, 4096, 0)
    }
    pub fn spawn(
        &mut self,
        entry_point: extern "C" fn(),
        stack_size: usize,
        priority: u8,
    ) -> Result<(), SchedulerError> {
        let task_id = self
            .task_list
            .iter()
            .position(|item| item.is_none())
            .map_or(self.task_list.len(), |pos| pos);

        let layout = Layout::from_size_align(stack_size, 16)?;
        let raw_ptr = unsafe { HEAP_ALLOCATOR.alloc(layout) }; // 起始地址
        let stack_ptr = NonNull::new(raw_ptr).ok_or(SchedulerError::MemoryAllocationError)?;
        let stack_base = stack_ptr.as_ptr() as usize;
        let stack_top = (stack_base + stack_size) & !0xF; // 栈顶（高地址，对齐）

        let mut task_context = TaskContext::zero();
        //对齐到16字节
        task_context.sp = stack_top;
        task_context.ra = trampoline as usize;
        task_context.a0 = entry_point as usize;
        task_context.mepc = trampoline as usize;
        //TODO: 若实现tls，则必须完成相关操作
        // task_context.ra = entry_point as *mut usize as usize;
        let tcb = TaskControlBlock {
            task_id,
            stack_base: stack_ptr,
            layout,
            entry_point,
            priority,
            status: TaskStatus::Ready,
            context: task_context,
        };
        if task_id == self.task_list.len() {
            self.task_list.push(Some(tcb));
        } else {
            self.task_list[task_id] = Some(tcb);
        }
        self.ready_queue.push_back(task_id);
        Ok(())
    }

    fn prepare_next_task(&mut self) -> *mut TaskContext {
        // 回收僵尸任务，但跳过当前任务（如果它刚退出的话）
        // 因为我们还在当前任务的栈上运行，不能立刻释放它
        let current_id = self.current_task_id;
        // polling_println!(
        //     "{:?},{:?},{:?}",
        //     self.current_task_id,
        //     self.ready_queue,
        //     self.task_list
        // );
        polling_println!("before: {:?}", self.current_task_id);
        // 一次性处理队列中的所有僵尸，遇到当前任务就提前终止循环
        let queue_len = self.zombie_queue.len();
        for _ in 0..queue_len {
            if let Some(zombie_id) = self.zombie_queue.pop() {
                if Some(zombie_id) == current_id {
                    // 是当前任务，放回队列末尾，停止处理
                    self.zombie_queue.push(zombie_id);
                    break;
                }
                // 不是当前任务，立刻回收
                self.ready_queue.retain(|&tid| tid != zombie_id);
                if let Some(slot) = self.task_list.get_mut(zombie_id) {
                    if let Some(tcb) = slot.take() {
                        unsafe {
                            HEAP_ALLOCATOR.dealloc(tcb.stack_base.as_ptr(), tcb.layout);
                        }
                    }
                }
            }
        }

        if let Some(cur) = current_id {
            if let Some(tcb) = self.task_list[cur].as_mut() {
                if matches!(tcb.status, TaskStatus::Running) {
                    tcb.status = TaskStatus::Ready;
                    self.ready_queue.push_back(cur);
                }
            }
        }

        // let next_id = self.ready_queue.pop_front().unwrap_or(0);
        // let next_ctx_ptr = {
        //     let next_tcb = self.task_list[next_id].as_mut().expect("next task missing");
        //     next_tcb.status = TaskStatus::Running;
        //     &mut next_tcb.context as *mut TaskContext
        // };
        let mut next_id = self.ready_queue.pop_front().unwrap_or(0);
        if next_id == 0 && !self.ready_queue.is_empty() {
            next_id = self.ready_queue.pop_front().unwrap();
            self.ready_queue.push_back(0);
        }
        let next_tcb = self.task_list[next_id].as_mut().expect("next task missing");
        next_tcb.status = TaskStatus::Running;
        self.current_task_id = Some(next_id);
        polling_println!("after: {:?}", self.current_task_id);
        &mut next_tcb.context as *mut TaskContext
        // unsafe {
        //     __switch_to(next_ctx_ptr);
        // }
    }
    pub fn mark_current_running(&mut self) {
        if let Some(id) = self.current_task_id {
            if let Some(tcb) = self.task_list[id].as_mut() {
                tcb.status = TaskStatus::Running;
            }
        }
    }
    fn exit_current_task() -> ! {
        // 为了确保除了第一个任务是由__switch_to切换的，其他的都由中断切换，这里只是加入僵尸队列然后空转等待中断调度
        // let next_ctx_ptr = {
        {
            let mut scheduler = SCHEDULER.lock();

            // 标记当前任务为已终止
            if let Some(id) = scheduler.current_task_id {
                if let Some(tcb) = scheduler.task_list[id].as_mut() {
                    tcb.status = TaskStatus::Terminated;
                    scheduler.zombie_queue.push(id);
                }
            }
        }
        // scheduler.prepare_next_task(true)
        // };
        // unsafe {
        // polling_println!("    ->aaa");

        loop {
            core::hint::spin_loop();
        }
        unreachable!();
    }
    pub fn run_scheduler() -> ! {
        // 获取第一个任务的上下文指针
        let next_ctx_ptr = {
            let mut scheduler = SCHEDULER.lock();
            scheduler.prepare_next_task()
        }; // 锁在这里被释放！

        // 在锁释放后执行上下文切换
        unsafe {
            __switch_to(next_ctx_ptr);
        }
        // polling_println!("here");
        unreachable!();
    }
    pub fn schedule_on_interrupt() -> *mut TaskContext {
        let mut scheduler = SCHEDULER.lock();
        scheduler.prepare_next_task()
    }
}

pub extern "C" fn trampoline(entry: extern "C" fn()) -> ! {
    {
        let mut scheduler = SCHEDULER.lock();
        scheduler.mark_current_running();
    }
    entry();
    Scheduler::exit_current_task(); // 通知调度器该任务结束，永不返回
}
extern "C" fn idle_task() {
    loop {
        core::hint::spin_loop();
        // println!("idle!");
    }
}
// #[unsafe(naked)]
// #[unsafe(no_mangle)]
// pub unsafe extern "C" fn idle() {
//     naked_asm!("idle_loop:", "wfi", "j idle_loop")
// }
