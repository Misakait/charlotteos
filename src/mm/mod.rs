// src/mm/mod.rs
pub mod address;
pub mod buddy;
pub mod bump;
pub mod mm_set;
pub mod pagetable;

use crate::config::PHYS_VIRT_OFFSET;
use crate::data_struct::sync_ref_cell::SyncRefCell;
use crate::mm::address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use crate::mm::buddy::phys_to_virt;
use crate::mm::bump::BumpAllocator;
use crate::mm::pagetable::{FrameTracker, PTEFlags, PageSize, PageTable};
use crate::println;
use buddy::BuddySystemFrameAllocator;
use buddy_system_allocator::LockedHeap;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::RefCell;
use core::num::NonZeroUsize;
use core::ptr;
use core::ptr::NonNull;
use lazy_static::lazy_static;
use spin::Mutex;
pub const PAGE_SIZE_BITS: usize = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_SIZE_BITS; // 4KB

// 定义一个带锁的分配器结构体
pub struct LockedFrameAllocator(Mutex<BuddySystemFrameAllocator>);

impl LockedFrameAllocator {
    /// 创建一个全局的、空的分配器实例
    pub const fn new() -> Self {
        Self(Mutex::new(BuddySystemFrameAllocator::new()))
    }

    /// 初始化堆，这个函数将由内核主程序调用
    pub unsafe fn init(&self, heap_start: usize, heap_end: usize) {
        self.0.lock().init(heap_start, heap_end);
    }
}
pub static FRAME_ALLOCATOR: LockedFrameAllocator = LockedFrameAllocator::new();

pub fn frame_alloc() -> Option<FrameTracker> {
    unsafe {
        FRAME_ALLOCATOR
            .0
            .lock()
            .alloc(NonZeroUsize::new_unchecked(1))
            .map(|ppn| FrameTracker { ppn })
    }
}

pub fn frame_dealloc(ppn: PhysPageNum) {
    unsafe {
        FRAME_ALLOCATOR
            .0
            .lock()
            .dealloc(ppn, NonZeroUsize::new_unchecked(1));
    }
}
// 为我们的带锁分配器实现 GlobalAlloc Trait
// unsafe impl GlobalAlloc for LockedAllocator {
//     unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
//         self.0
//             .lock()
//             .alloc(layout)
//             .map_or(ptr::null_mut(), |p| p.as_ptr())
//     }

//     unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
//         self.0.lock().dealloc(NonNull::new_unchecked(ptr), layout);
//     }
// }
// 使用 #[global_allocator] 属性来注册我们的全局分配器实例
#[global_allocator]
// pub static HEAP_ALLOCATOR: LockedAllocator = LockedAllocator::new();
pub static HEAP_ALLOCATOR: LockedHeap<32> = LockedHeap::empty();
// #[alloc_error_handler]
// pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
//     panic!("Heap allocation error, layout = {:?}", layout);
// }
unsafe extern "C" {
    static _ekernel: usize;
    static _heap_size: usize;
    static _text_start: usize;
    static _text_end: usize;
    static _rodata_start: usize;
    static _rodata_end: usize;
    static _data_start: usize;
    static _data_end: usize;
    static _bss_start: usize;
    static _bss_end: usize;
    static _memory_end: usize;
    static _memory_start: usize;
}

lazy_static! {
    static ref BUMP_ALLOCATOR: SyncRefCell<BumpAllocator> = {
        let ekernel = unsafe { &_ekernel as *const _ as usize };
        let memory_end = unsafe { &_memory_end as *const _ as usize };
        unsafe { SyncRefCell::new(BumpAllocator::new(ekernel, memory_end)) }
    };
}
static BOOT_ROOT_PPN: SyncRefCell<PhysPageNum> = unsafe { SyncRefCell::new(PhysPageNum(0)) };
// pub static BUMP_ALLOCATOR: SyncRefCell<BumpAllocator> = {
//     let ekernel = unsafe { &_ekernel as *const _ as usize };
//     let memory_end = unsafe { &_memory_end as *const _ as usize };
//     BumpAllocator::new(ekernel, memory_end)
// };

fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

fn align_down(value: usize, align: usize) -> usize {
    value & !(align - 1)
}

pub fn boot_bump_map() {
    let memory_start = unsafe { &_memory_start as *const _ as usize };
    let memory_end = unsafe { &_memory_end as *const _ as usize };
    let root_ppn = BUMP_ALLOCATOR
        .borrow_mut()
        .alloc_page()
        .expect("boot root page table allocation failed");
    let root_pa = PhysAddr::from(&root_ppn).0;

    let root_pt = unsafe { &mut *(root_pa as *mut PageTable) };

    let flags = PTEFlags::R | PTEFlags::W | PTEFlags::X;
    let page_start = align_down(memory_start, PAGE_SIZE);
    let page_end = align_up(memory_end, PAGE_SIZE);
    let aligned_start = align_up(page_start, 0x20_0000);
    let aligned_end = align_down(page_end, 0x20_0000);

    let head_end = aligned_start.min(page_end);
    let mut current_pa = page_start;
    while current_pa < head_end {
        let va = current_pa + PHYS_VIRT_OFFSET;
        let vpn = VirtAddr::from(va);
        root_pt.bump_map(
            VirtPageNum::from(vpn),
            PhysPageNum::from(PhysAddr(current_pa)),
            flags,
            PageSize::FourKB,
        );
        current_pa += PAGE_SIZE;
    }

    let mut current_pa = aligned_start;
    let mid_end = aligned_end.min(page_end);
    while current_pa < mid_end {
        let va = current_pa + PHYS_VIRT_OFFSET;
        let vpn = VirtAddr::from(va);
        root_pt.bump_map(
            VirtPageNum::from(vpn),
            PhysPageNum::from(PhysAddr(current_pa)),
            flags,
            PageSize::TwoMB,
        );
        current_pa += 0x20_0000;
    }

    let mut current_pa = mid_end;
    while current_pa < page_end {
        let va = current_pa + PHYS_VIRT_OFFSET;
        let vpn = VirtAddr::from(va);
        root_pt.bump_map(
            VirtPageNum::from(vpn),
            PhysPageNum::from(PhysAddr(current_pa)),
            flags,
            PageSize::FourKB,
        );
        current_pa += PAGE_SIZE;
    }

    *BOOT_ROOT_PPN.borrow_mut() = root_ppn;
}

pub fn init_heap() {
    const HEAP_SIZE: usize = 0x20_0000;
    const HEAP_PAGES: usize = HEAP_SIZE / PAGE_SIZE;

    println!("Initializing Heap Allocator...");

    // 2. 向物理页分配器申请连续的物理页
    // 这里调用的是你之前写的 BuddySystemFrameAllocator::alloc
    let start_ppn = FRAME_ALLOCATOR
        .0
        .lock()
        .alloc(NonZeroUsize::new(HEAP_PAGES).unwrap())
        .expect("Failed to allocate physical memory for Heap!");

    // 3. 计算物理地址和虚拟地址
    let start_pa = start_ppn.0 * PAGE_SIZE; // 物理页号转物理地址
    let start_va = phys_to_virt(start_pa); // 物理地址转虚拟地址

    println!(
        " -> Heap range: PA: {:#x} => VA: {:#x}, Size: {} MB",
        start_pa,
        start_va,
        HEAP_SIZE / 1024 / 1024
    );

    // 4. 初始化堆分配器
    // 注意：init 接收的必须是 [虚拟地址]
    unsafe {
        HEAP_ALLOCATOR.lock().init(start_va, HEAP_SIZE);
    }
}
// /// 初始化堆分配器
// pub fn init_heap() {
//     // 获取原始的堆边界
//     let heap_start_raw = unsafe { &_heap_start as *const _ as usize };
//     let heap_end_raw = unsafe { &_memory_end as *const _ as usize };

//     // 对齐堆的起始地址
//     //    `next_multiple_of` 可以将地址向上对齐到最近的页面边界
//     // let heap_start = heap_start_raw.next_multiple_of(PAGE_SIZE);
//     unsafe {
//         HEAP_ALLOCATOR.init(heap_start_raw, heap_end_raw);
//     }
// }
