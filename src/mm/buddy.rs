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

        // 1. 根据请求的 layout，计算出需要的最小块的阶 (order)
        //    这里要考虑 layout 的大小和对齐要求
        let required_size = layout.size().max(layout.align());
        let required_order = size_to_order(required_size);

        if required_order >= MAX_ORDER {
            return None; // 请求过大
        }

        // 2. 寻找一个合适的空闲块，从需要的阶开始，向上查找
        let mut order = required_order;
        while order < MAX_ORDER {
            // 如果当前阶的空闲链表不为空，我们就找到了
            if self.free_lists[order].is_some() {
                // --- 找到了足够大的块，开始处理 ---

                // a. 从链表中移除这个块
                let block = self.free_lists[order].take().unwrap();
                unsafe {
                    // 将链表头更新为下一个节点
                    self.free_lists[order] = (*block.as_ptr()).next.take();
                }

                // b. 开始循环分裂，直到块的大小刚刚好
                let mut current_order = order;
                while current_order > required_order {
                    // 计算分裂后的伙伴块的地址和大小
                    let current_block_size = 1 << (current_order + 12); // PAGE_SIZE 2^12
                    let buddy_block_size = current_block_size / 2;

                    let buddy_addr = block.as_ptr() as usize + buddy_block_size;

                    // 将分裂出的伙伴块加回到系统中
                    unsafe {
                        self.add_free_block(buddy_addr, buddy_block_size);
                    }

                    current_order -= 1;
                }
                println!("the allocated block at 0x{:x} with size {} ({} KB, {} MB)", block.as_ptr() as usize, required_size, required_size / 1024, required_size / 1024 / 1024);
                // c. 返回最终大小合适的块
                return Some(block.cast());
            }

            // 如果当前阶为空，就去更高一阶查找
            order += 1;
        }

        // 如果所有阶都找遍了还是没有，说明内存不足
        None
    }

    /// 释放内存
    pub fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) {
        // ... 伙伴系统的核心释放逻辑 ...
        // 1. 根据释放的地址和大小，计算其阶 (order)。
        // 2. 将其放入对应阶的空闲链表。
        // 3. 循环检查：它的“伙伴”块是否也在空闲链表中。
        // 4. 如果伙伴也空闲，就将两者合并成一个更大的块，放入更高阶的链表中，并继续向上检查合并。
        // 1. 根据 layout 计算出要释放的块的大小和阶 (order)
        let required_size = layout.size().max(layout.align());
        let order = size_to_order(required_size);

        // 我们实际分配的块大小
        let mut block_size = 1 << (order + 12); // PAGE_SIZE 2^12
        let mut block_addr = ptr.as_ptr() as usize;

        // 2. 开始循环，尝试与伙伴合并
        let mut current_order = order;
        while current_order < MAX_ORDER - 1 {
            let buddy_addr = block_addr ^ block_size;

            // 现在，我们只调用一次辅助函数，它完成了查找和移除两个任务
            unsafe {
                if self.try_remove_from_list(buddy_addr, current_order) {
                    // 如果成功移除了伙伴，则进行合并
                    block_addr = block_addr.min(buddy_addr);
                    block_size *= 2;
                    current_order += 1;
                } else {
                    // 如果没有找到空闲的伙伴，就停止合并
                    break;
                }
            }
        }

        unsafe { self.add_free_block(block_addr, block_size); }
        // while current_order < MAX_ORDER - 1 {
        //     // a. 计算伙伴块的地址
        //     //    伙伴地址可以通过将当前地址与块大小进行异或(XOR)运算得到
        //     let buddy_addr = block_addr ^ block_size;
        //     let mut previous_addr: Option<NonNull<ListNode>> = None;
        //     // b. 在对应阶的空闲链表中，查找是否存在这个伙伴块
        //     let buddy_is_free = self.free_lists[current_order]
        //         .map_or(false, |mut list_head| {
        //             // --- 改进后的遍历逻辑 ---
        //             loop {
        //                 // 首先，检查当前节点
        //                 if list_head.as_ptr() as usize == buddy_addr {
        //                     match previous_addr {
        //                         None => {
        //                             let next_ptr = unsafe { (*list_head.as_ptr()).next };
        //                             // 如果是链表头，直接更新头指针
        //                             self.free_lists[current_order] = next_ptr;
        //                         }
        //                         Some(ptr) => {
        //                             let next_ptr = unsafe { (*list_head.as_ptr()).next };
        //                             // 如果是中间节点，更新前一个节点的 next 指针
        //                             unsafe {
        //                                 write_volatile(&mut (*ptr.as_ptr()).next, next_ptr);
        //                             }
        //                         }
        //                     }
        //                     return true; // 找到了！
        //                 }
        //                 // 如果当前节点不是我们要找的，就继续遍历
        //                 // 然后，尝试移动到下一个节点
        //                 if let Some(next_node) = unsafe { (*list_head.as_ptr()).next } {
        //                     previous_addr = Some(list_head);
        //                     list_head = next_node;
        //                 } else {
        //                     // 如果没有下一个节点了，说明已经遍历完且没找到
        //                     return false;
        //                 }
        //             }
        //             // --- 逻辑结束 ---
        //         });
        //
        //     // c. 如果伙伴块不是空闲的，或者已经被合并，就停止合并
        //     if !buddy_is_free {
        //         break;
        //     }
        //
        //     // d. 如果伙伴块是空闲的，就将它从空闲链表中移除，准备合并
        //     // self.remove_from_free_list(buddy_addr, current_order);
        //
        //     // e. 合并：更新当前块的地址为两个伙伴中较小的那个，大小翻倍
        //     block_addr = block_addr.min(buddy_addr);
        //     // block_size *= 2; // 等同于下面的操作
        //     current_order += 1; // 阶数加一，进入下一轮循环，尝试与新的、更大的伙伴合并
        // }
        //
        // // 3. 将最终合并好的（或未合并的）块，加入到对应阶的空闲链表中
        // unsafe { self.add_free_block(block_addr, 1 << (current_order + 12)); }
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

    unsafe fn try_remove_from_list(&mut self, addr: usize, order: usize) -> bool {
        let list_head = match self.free_lists[order] {
            Some(head) => head,
            None => return false, // 链表为空，直接返回
        };

        // Case 1: 要移除的块就是头节点
        if list_head.as_ptr() as usize == addr {
            self.free_lists[order] = (*list_head.as_ptr()).next.take();
            return true;
        }

        // Case 2: 遍历链表查找
        let mut current = list_head;
        while let Some(mut next_node) = (*current.as_ptr()).next {
            if next_node.as_ptr() as usize == addr {
                // 找到了，让当前节点的 next 直接指向下一个节点的 next
                (*current.as_ptr()).next = (*next_node.as_ptr()).next.take();
                return true;
            }
            current = next_node;
        }

        // 遍历完整个链表都没找到
        false
    }
}
/// 辅助函数：根据请求的大小计算出需要的阶
fn size_to_order(size: usize) -> usize {
    if size == 0 {
        return 0;
    }
    // 向上取整到最近的 2 的幂次方，然后计算阶
    let power_of_2_size = size.next_power_of_two();
    // 减 12 是因为我们的 order 0 对应 4KB (2^12)
    (power_of_2_size.trailing_zeros() as usize).saturating_sub(12)
}
unsafe impl Send for BuddySystemAllocator {}