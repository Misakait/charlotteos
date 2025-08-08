use core::alloc::Layout;
use core::ptr::{write_volatile, NonNull};
use crate::mm::{HEAP_ALLOCATOR, PAGE_SIZE};
use crate::println;

pub const MAX_ORDER: usize = 16; // 阶数范围是 0..15，共 16 个
#[repr(C)]
struct ListNode {
    next: Option<NonNull<ListNode>>,
}
impl ListNode {
    fn new() -> ListNode {
        ListNode { next: None }
    }
}

/// 伙伴系统分配器
pub struct BuddySystemAllocator {
    free_lists: [Option<NonNull<ListNode>>; MAX_ORDER], // 按2的幂次管理空闲链表
    heap_start: usize,
    heap_end: usize,
}
impl BuddySystemAllocator {
    /// 创建一个空的、未初始化的分配器
    pub const fn new() -> Self {
        Self {
            free_lists: [None; MAX_ORDER],
            heap_start: 0,
            heap_end: 0,
        }
    }

    /// 初始化分配器
    /// heap_start 和 heap_end 必须是页对齐的
    pub unsafe fn init(&mut self, heap_start: usize, heap_end: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_end;
        // unsafe {
        //     // 将整个堆空间作为最大的一块放入空闲链表
        //     self.add_free_block(heap_start, heap_end - heap_start);
        // }
        let mut current_start = heap_start;
        println!("Buddy System Allocator initialized:");
        println!("  -> Heap start: 0x{:x}, end: 0x{:x}", heap_start, heap_end);
        while current_start < heap_end {
            // a. 计算剩余空间大小
            let remaining_size = heap_end - current_start;

            // b. 找出能放入剩余空间的最大2次幂块
            let block_size = if remaining_size == 0 { 0 } else { 1 << remaining_size.ilog2() };
            // c. 确保块大小不小于最小单位 (PAGE_SIZE)
            if block_size < PAGE_SIZE {
                break;
            }
            // d. 将这个块交给伙伴系统
            unsafe {
                self.add_free_block(current_start, block_size);
            }
            println!("  -> Added block at 0x{:x},end at 0x{:x} size 0x{:x} ({} KB, {}MB)", current_start, current_start + block_size , block_size, block_size / 1024,block_size / 1024/ 1024  );
            // e. 更新下一次循环的起点
            current_start += block_size;
        }
    }

    /// 分配内存
    pub fn alloc(&mut self, layout: Layout) -> Option<NonNull<u8>> {
        // ... 伙伴系统的核心分配逻辑 ...
        // 1. 根据请求大小，计算需要的块大小 (2的幂次) 和对应的阶 (order)。
        // 2. 在对应阶的空闲链表中查找可用块。
        // 3. 如果找不到，就去更高阶的链表中找，然后进行分裂。
        // 4. 分裂出的多余“伙伴”块，放入对应低阶的空闲链表中。
        // 5. 返回找到的块的指针。
        // (为简洁起见，此处省略具体实现)
        None // 占位
    }

    /// 释放内存
    pub fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) {
        // ... 伙伴系统的核心释放逻辑 ...
        // 1. 根据释放的地址和大小，计算其阶 (order)。
        // 2. 将其放入对应阶的空闲链表。
        // 3. 循环检查：它的“伙伴”块是否也在空闲链表中。
        // 4. 如果伙伴也空闲，就将两者合并成一个更大的块，放入更高阶的链表中，并继续向上检查合并。
        // (为简洁起见，此处省略具体实现)
    }

    // 辅助函数，用于将空闲块添加到链表
    pub(crate) unsafe fn add_free_block(&mut self, addr: usize, size: usize) {

        // --- 1. 计算阶 (Order) ---
        // size.trailing_zeros() 是一个计算 log2(size) 的高效方法
        // 例如 4096 (2^12) 的 trailing_zeros 就是 12
        // 假设我们的 order 0 对应 4KB (2^12 字节)，所以需要减去 12
        let order = size.trailing_zeros() as usize - 12; // PAGE_SIZE 是 4KB (2^12)
        if order >= MAX_ORDER {
            // 如果计算出的阶超出了我们的管理范围，进行处理
            panic!("Requested size is too large for buddy system allocator");
        }
        // --- 2. 链表头插法 ---
        // a. 读取当前阶的链表头
        let old_head = self.free_lists[order].take(); // .take() 会取出 Some(T) 并留下 None

        // b. 将新节点的 next 指向旧的头节点
        // (*new_node_ptr).next = old_head;
        // 使用 write_volatile 更能体现底层操作的意图
        unsafe {
            let new_node_ptr = addr as *mut ListNode;
            write_volatile(&mut (*new_node_ptr).next, old_head);

            // c. 更新链表头为新节点
            //    NonNull::new_unchecked 假设 ptr 永不为 null，在这里是安全的
            self.free_lists[order] = Some(NonNull::new_unchecked(new_node_ptr));
        }
    }
}
unsafe impl Send for BuddySystemAllocator {}