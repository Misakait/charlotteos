use crate::{
    mm::{
        BUDDY_ALLOCATOR, PAGE_SIZE,
        address::{PhysAddr, PhysPageNum},
        buddy::{phys_to_virt, virt_to_phys},
    },
    polling_println, println,
    task::{
        SCHEDULER,
        context::TaskContext,
        switch::first_switch_to,
        tcb::{TaskControlBlock, TaskStatus},
    },
    trap::{
        interrupts::{init_supervisor_interrupts, set_next_timer_tick},
        trap_handler,
    },
    userlib::syscall::sys_task_exit,
};
use alloc::{
    boxed::Box,
    collections::{binary_heap::BinaryHeap, vec_deque::VecDeque},
    vec::Vec,
};
use core::{
    alloc::{GlobalAlloc, Layout, LayoutError},
    arch::asm,
    cmp::Reverse,
    mem::transmute,
    num::NonZeroUsize,
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
    wake_time: usize,
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
    pub fn get_current_task_id(&self) -> TaskId {
        self.current_task_id.unwrap()
    }
    pub fn get_zombie_queue(&mut self) -> &mut Vec<TaskId> {
        &mut self.zombie_queue
    }
    pub fn get_task_list(&mut self) -> &mut Vec<Option<TaskControlBlock>> {
        &mut self.task_list
    }
    pub fn init() -> Result<(), SchedulerError> {
        let mut scheduler = SCHEDULER.lock();
        scheduler.spawn(idle_task, 4096, 0)
    }

    pub fn spawn<F>(
        &mut self,
        task: F,
        stack_size: usize,
        priority: u8,
    ) -> Result<(), SchedulerError>
    where
        F: FnOnce() + Send + 'static,
    {
        let task_id = self
            .task_list
            .iter()
            .position(|item| item.is_none())
            .map_or(self.task_list.len(), |pos| pos);

        let pages = (stack_size + PAGE_SIZE - 1) / PAGE_SIZE;
        let non_zero_pages =
            NonZeroUsize::new(pages).ok_or(SchedulerError::MemoryAllocationError)?;
        let stack_ppn = BUDDY_ALLOCATOR
            .lock()
            .alloc(non_zero_pages)
            .ok_or(SchedulerError::MemoryAllocationError)?;
        // 转换成虚拟地址
        let stack_base_pa = PhysAddr::from(&stack_ppn).0;
        let stack_base_va = phys_to_virt(stack_base_pa);
        let stack_top = stack_base_va + pages * PAGE_SIZE;

        let stack_ptr = NonNull::new(stack_base_va as *mut u8).unwrap();
        // let layout = Layout::from_size_align(stack_size, 16)?;
        // let raw_ptr = unsafe { HEAP_ALLOCATOR.alloc(layout) }; // 起始地址
        // let stack_ptr = NonNull::new(raw_ptr).ok_or(SchedulerError::MemoryAllocationError)?;
        // let stack_base = stack_ptr.as_ptr() as usize;
        // let stack_top = (stack_base + stack_size) & !0xF; // 栈顶（高地址，对齐）

        let task_box: Box<dyn FnOnce() + Send> = Box::new(task);
        let raw_fat_ptr = Box::into_raw(task_box);

        // Rust 的胖指针布局通常是 (data_ptr, vtable_ptr)
        // 将其强转为 (usize, usize) 元组
        let (data_ptr, vtable_ptr): (usize, usize) = unsafe { transmute(raw_fat_ptr) };

        let mut task_context = TaskContext::zero();
        task_context.sp = stack_top;
        task_context.ra = trampoline as usize;
        task_context.a0 = data_ptr;
        task_context.a1 = vtable_ptr;
        task_context.sepc = trampoline as usize;
        //TODO: 若实现tls，则必须完成相关操作
        // task_context.ra = entry_point as *mut usize as usize;
        let tcb = TaskControlBlock {
            task_id,
            stack_base: stack_ptr,
            page_count: pages,
            entry_point: (data_ptr, vtable_ptr),
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
    pub fn set_current_task_sleep(&mut self, wake_time: usize) -> *mut TaskContext {
        let current_id = self.current_task_id;

        if let Some(cur) = current_id {
            if let Some(tcb) = self.task_list[cur].as_mut() {
                if matches!(tcb.status, TaskStatus::Running) {
                    tcb.status = TaskStatus::Blocked;
                    self.blocked_queue.push(Reverse(SleepEntry {
                        wake_time,
                        task_id: cur,
                    }));
                    let mut next_id = self.ready_queue.pop_front().unwrap_or(0);
                    if next_id == 0 && !self.ready_queue.is_empty() {
                        next_id = self.ready_queue.pop_front().unwrap();
                        self.ready_queue.push_back(0);
                    }
                    let next_tcb = self.task_list[next_id].as_mut().expect("next task missing");
                    next_tcb.status = TaskStatus::Running;
                    self.current_task_id = Some(next_id);
                    return &mut next_tcb.context as *mut TaskContext;
                }
            }
        }
        unreachable!();
    }
    pub fn set_task_ready(&mut self, task_id: usize) {
        if let Some(tcb) = self.task_list[task_id].as_mut() {
            if matches!(tcb.status, TaskStatus::Blocked) {
                // 这里不删除堆中的元素，留给finish_sleep自动pop出去
                tcb.status = TaskStatus::Ready;
                self.ready_queue.push_back(task_id);
            }
        }
    }
    pub fn wake_up_task_with_result(&mut self, task_id: usize, result: u8) {
        if let Some(tcb) = self.task_list[task_id].as_mut() {
            if matches!(tcb.status, TaskStatus::Blocked) {
                tcb.status = TaskStatus::Ready;
                tcb.context.a0 = result as usize; // 将字符写入任务上下文的 a0
                self.ready_queue.push_back(task_id);
            }
        }
    }
    pub fn block_current_task(&mut self) -> *mut TaskContext {
        let current_id = self.current_task_id;

        if let Some(cur) = current_id {
            if let Some(tcb) = self.task_list[cur].as_mut() {
                if matches!(tcb.status, TaskStatus::Running) {
                    tcb.status = TaskStatus::Blocked;
                    let mut next_id = self.ready_queue.pop_front().unwrap_or(0);
                    if next_id == 0 && !self.ready_queue.is_empty() {
                        next_id = self.ready_queue.pop_front().unwrap();
                        self.ready_queue.push_back(0);
                    }
                    let next_tcb = self.task_list[next_id].as_mut().expect("next task missing");
                    next_tcb.status = TaskStatus::Running;
                    self.current_task_id = Some(next_id);
                    return &mut next_tcb.context as *mut TaskContext;
                }
            }
        }
        unreachable!();
    }
    pub fn finish_sleep(&mut self, current_mtime: usize) {
        loop {
            if let Some(Reverse(entry)) = self.blocked_queue.peek() {
                if entry.wake_time > current_mtime {
                    break;
                }
            } else {
                break;
            }
            // 堆顶任务 wake_time <= current_mtime
            let Reverse(entry) = self.blocked_queue.pop().unwrap();
            if let Some(tcb) = self.task_list[entry.task_id].as_mut() {
                if matches!(tcb.status, TaskStatus::Blocked) {
                    tcb.status = TaskStatus::Ready;
                    self.ready_queue.push_back(entry.task_id);
                }
            }
        }
    }
    fn prepare_next_task(&mut self) -> *mut TaskContext {
        // 回收僵尸任务，但跳过当前任务（如果它刚退出的话）z
        // 因为我们还在当前任务的栈上运行，不能立刻释放它
        let current_id = self.current_task_id;
        // polling_println!(
        //     "{:?},{:?},{:?}",
        //     self.current_task_id,
        //     self.ready_queue,
        //     self.task_list
        // );
        // polling_println!("before: {:?}", self.current_task_id);
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
                            let stack_va = tcb.stack_base.as_ptr() as usize;
                            let stack_pa = PhysAddr(virt_to_phys(stack_va));
                            let stack_ppn = PhysPageNum::from(stack_pa);
                            BUDDY_ALLOCATOR
                                .lock()
                                .dealloc(stack_ppn, NonZeroUsize::new(tcb.page_count).unwrap());
                            // HEAP_ALLOCATOR.dealloc(tcb.stack_base.as_ptr(), tcb.layout);
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
        // polling_println!("after: {:?}", self.current_task_id);
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

    pub fn run_scheduler() -> ! {
        // 获取第一个任务的上下文指针
        let next_ctx_ptr = {
            let mut scheduler = SCHEDULER.lock();
            scheduler.prepare_next_task()
        }; // 锁在这里被释放！
        // polling_println!("here");
        // 在锁释放后执行上下文切换
        unsafe {
            first_switch_to(next_ctx_ptr);
        }
        // polling_println!("here");
        unreachable!();
    }
    pub fn schedule_on_interrupt() -> *mut TaskContext {
        let mut scheduler = SCHEDULER.lock();
        scheduler.prepare_next_task()
    }
}

pub extern "C" fn trampoline(data_ptr: usize, vtable_ptr: usize) -> ! {
    // {
    //     let mut scheduler = SCHEDULER.lock();
    //     scheduler.mark_current_running();
    // }
    unsafe {
        let raw_fat_ptr: *mut (dyn FnOnce() + Send + 'static) = transmute((data_ptr, vtable_ptr));
        let task = Box::from_raw(raw_fat_ptr);
        task();
    }
    sys_task_exit();
    unreachable!();
    // Scheduler::exit_current_task(); // 通知调度器该任务结束，永不返回
}
fn idle_task() {
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
