use crate::task::tcp::TaskControlBlock;
use alloc::{
    collections::{binary_heap::BinaryHeap, vec_deque::VecDeque},
    vec::Vec,
};
use core::cmp::Reverse;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SleepEntry {
    wake_tick: usize,
    task_id: usize,
}

type TaskId = usize;
struct Scheduler {
    ready_queue: VecDeque<TaskId>,
    task_list: Vec<Option<TaskControlBlock>>,
    blocked_queue: BinaryHeap<Reverse<SleepEntry>>,
}
