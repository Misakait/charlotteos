// src/mm/mod.rs
pub mod buddy;

use crate::println;
use buddy::BuddySystemAllocator;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use core::ptr::NonNull;
use spin::Mutex;

pub const PAGE_SIZE: usize = 4096; // 4KB

// 定义一个带锁的分配器结构体
pub struct LockedAllocator(Mutex<BuddySystemAllocator>);

impl LockedAllocator {
    /// 创建一个全局的、空的分配器实例
    pub const fn new() -> Self {
        Self(Mutex::new(BuddySystemAllocator::new()))
    }

    /// 初始化堆，这个函数将由内核主程序调用
    pub unsafe fn init(&self, heap_start: usize, heap_end: usize) {
        self.0.lock().init(heap_start, heap_end);
    }
}

// 为我们的带锁分配器实现 GlobalAlloc Trait
unsafe impl GlobalAlloc for LockedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0
            .lock()
            .alloc(layout)
            .map_or(ptr::null_mut(), |p| p.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0.lock().dealloc(NonNull::new_unchecked(ptr), layout);
    }
}
// 使用 #[global_allocator] 属性来注册我们的全局分配器实例
#[global_allocator]
pub static HEAP_ALLOCATOR: LockedAllocator = LockedAllocator::new();

unsafe extern "C" {
    static _heap_start: usize;
    static _heap_size: usize;
    static _text_start: usize;
    static _memory_end: usize;
}
/// 初始化堆分配器
pub fn init_heap() {
    // 获取原始的堆边界
    let heap_start_raw = unsafe { &_heap_start as *const _ as usize };
    let heap_end_raw = unsafe { &_memory_end as *const _ as usize };

    // 对齐堆的起始地址
    //    `next_multiple_of` 可以将地址向上对齐到最近的页面边界
    let heap_start = heap_start_raw.next_multiple_of(PAGE_SIZE);
    unsafe {
        HEAP_ALLOCATOR.init(heap_start, heap_end_raw);
    }
}
