use crate::config::PHYS_VIRT_OFFSET;
use crate::mm::address::{PhysAddr, PhysPageNum};
use crate::mm::{PAGE_SIZE, PAGE_SIZE_BITS};
use crate::println;
use core::num::NonZeroUsize;
use core::ptr::{write_volatile, NonNull};

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
pub struct BuddySystemFrameAllocator {
    free_lists: [Option<NonNull<ListNode>>; MAX_ORDER], // 按2的幂次管理空闲链表
    start: PhysPageNum,
    end: PhysPageNum,
}
impl BuddySystemFrameAllocator {
    /// 创建一个空的、未初始化的分配器
    pub const fn new() -> Self {
        Self {
            free_lists: [None; MAX_ORDER],
            start: PhysPageNum(0),
            end: PhysPageNum(0),
        }
    }

    /// 初始化分配器
    /// pa_start 和 pa_end 必须是页对齐的
    pub unsafe fn init(&mut self, pa_start: usize, pa_end: usize) {
        let start_addr = PhysAddr(pa_start);
        let end_addr = PhysAddr(pa_end);
        // start 必须向上取整 (ceil)，因为如果不满一页，那半页不能用
        self.start = start_addr.ceil();
        // end 必须向下取整 (floor)，防止越界到非法的内存去
        self.end = end_addr.floor();
        let mut current_ppn = self.start.0;
        let end_ppn = self.end.0;
        println!("Buddy System Allocator initialized:");
        println!("  -> Heap start: 0x{:x}, end: 0x{:x}", pa_start, pa_end);
        while current_ppn < end_ppn {
            let remaining_pages = end_ppn - current_ppn;
            if remaining_pages == 0 {
                break;
            }

            let max_order_by_remaining = remaining_pages.ilog2() as usize;
            let max_order_by_alignment = if current_ppn == 0 {
                MAX_ORDER - 1
            } else {
                (current_ppn.trailing_zeros() as usize).min(MAX_ORDER - 1)
            };
            let order = max_order_by_remaining
                .min(max_order_by_alignment)
                .min(MAX_ORDER - 1);
            let block_pages = 1usize << order;
            let block_addr = PhysAddr::from(&PhysPageNum(current_ppn));
            let block_size = block_pages * PAGE_SIZE;

            unsafe {
                self.add_free_block(block_addr.0, order);
            }
            println!(
                "  -> Added block at 0x{:x},end at 0x{:x} size 0x{:x} ({} KB, {}MB)",
                block_addr.0,
                block_addr.0 + block_size,
                block_size,
                block_size / 1024,
                block_size / 1024 / 1024
            );
            current_ppn += block_pages;
        }
    }

    /// 分配内存
    pub fn alloc(&mut self, pages: NonZeroUsize) -> Option<PhysPageNum> {
        // ... 伙伴系统的核心分配逻辑 ...
        // 1. 根据请求大小，计算需要的块大小 (2的幂次) 和对应的阶 (order)。
        // 2. 在对应阶的空闲链表中查找可用块。
        // 3. 如果找不到，就去更高阶的链表中找，然后进行分裂。
        // 4. 分裂出的多余“伙伴”块，放入对应低阶的空闲链表中。
        // 5. 返回找到的块的指针。

        let required_order = pages_to_order(pages);

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
                let block_pa = self.free_lists[order].take().unwrap();
                let block_va = phys_to_virt(block_pa.as_ptr() as usize) as *mut ListNode;
                unsafe {
                    // 将链表头更新为下一个节点
                    self.free_lists[order] = (*block_va).next.take();
                }

                // b. 开始循环分裂，直到块的大小刚刚好
                let mut current_order = order;
                let block_addr = block_pa.as_ptr() as usize;
                while current_order > required_order {
                    // 计算分裂后的伙伴块的地址和大小
                    let current_block_size = 1usize << (current_order + PAGE_SIZE_BITS);
                    let buddy_block_size = current_block_size / 2;

                    let buddy_addr = block_addr + buddy_block_size;

                    // 将分裂出的伙伴块加回到系统中
                    unsafe {
                        self.add_free_block(buddy_addr, current_order - 1);
                    }

                    current_order -= 1;
                }
                // println!(
                //     "the allocated block at 0x{:x} with size {} ({} KB, {} MB)",
                //     block.as_ptr() as usize,
                //     required_size,
                //     required_size / 1024,
                //     required_size / 1024 / 1024
                // );
                // c. 返回最终大小合适的块
                return Some(PhysAddr(block_addr).into());
            }

            // 如果当前阶为空，就去更高一阶查找
            order += 1;
        }

        // 如果所有阶都找遍了还是没有，说明内存不足
        None
    }

    /// 释放内存
    pub fn dealloc(&mut self, ppn: PhysPageNum, pages: NonZeroUsize) {
        // ... 伙伴系统的核心释放逻辑 ...
        // 1. 根据释放的地址和大小，计算其阶 (order)。
        // 2. 将其放入对应阶的空闲链表。
        // 3. 循环检查：它的“伙伴”块是否也在空闲链表中。
        // 4. 如果伙伴也空闲，就将两者合并成一个更大的块，放入更高阶的链表中，并继续向上检查合并。
        // if pages == 0 {
        //     return;
        // }

        let order = pages_to_order(pages);

        // 我们实际分配的块大小
        let mut block_size = 1usize << (order + PAGE_SIZE_BITS);
        let mut block_addr = PhysAddr::from(&ppn).0;

        // 2. 开始循环，尝试与伙伴合并
        let mut current_order = order;
        while current_order < MAX_ORDER - 1 {
            let buddy_addr = block_addr ^ block_size;

            // 现在，我们只调用一次辅助函数，它完成了查找和移除两个任务
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

        unsafe {
            self.add_free_block(block_addr, current_order);
        }
    }

    // 辅助函数，用于将空闲块添加到链表
    pub(crate) unsafe fn add_free_block(&mut self, phys_addr: usize, order: usize) {
        // --- 1. 计算阶 (Order) ---
        // size.trailing_zeros() 是一个计算 log2(size) 的高效方法
        // 例如 4096 (2^12) 的 trailing_zeros 就是 12
        // 假设我们的 order 0 对应 4KB (2^12 字节)，所以需要减去 12
        // let order = size.trailing_zeros() as usize - PAGE_SIZE_BITS; // PAGE_SIZE 是 4KB (2^12)
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
            let new_node_pa = phys_addr as *mut ListNode;
            let new_node_va = phys_to_virt(new_node_pa as usize) as *mut ListNode;
            write_volatile(&mut (*new_node_va).next, old_head);

            // c. 更新链表头为新节点
            //    NonNull::new_unchecked 假设 ptr 永不为 null，在这里是安全的
            self.free_lists[order] = Some(NonNull::new_unchecked(new_node_pa));
        }
    }

    fn try_remove_from_list(&mut self, phys_addr: usize, order: usize) -> bool {
        let list_head = match self.free_lists[order] {
            Some(head) => head,
            None => return false, // 链表为空，直接返回
        };
        unsafe {
            // Case 1: 要移除的块就是头节点
            if list_head.as_ptr() as usize == phys_addr {
                let list_head_va = phys_to_virt(list_head.as_ptr() as usize) as *mut ListNode;
                self.free_lists[order] = (*list_head_va).next.take();
                return true;
            }

            // Case 2: 遍历链表查找
            let current = list_head;
            let mut current_va = phys_to_virt(current.as_ptr() as usize) as *mut ListNode;
            while let Some(next_node) = (*current_va).next {
                let next_node_va = phys_to_virt(next_node.as_ptr() as usize) as *mut ListNode;
                if next_node.as_ptr() as usize == phys_addr {
                    // 找到了，让当前节点的 next 直接指向下一个节点的 next
                    (*current_va).next = (*next_node_va).next.take();
                    return true;
                }
                current_va = next_node_va;
            }
        }
        // 遍历完整个链表都没找到
        false
    }
}
/// 辅助函数：根据请求的页数计算出需要的阶
fn pages_to_order(pages: NonZeroUsize) -> usize {
    // if pages == 0 {
    //     return 0;
    // }
    let power_of_2_pages = pages.get().next_power_of_two();
    power_of_2_pages.trailing_zeros() as usize
}
unsafe impl Send for BuddySystemFrameAllocator {}
pub fn phys_to_virt(pa: usize) -> usize {
    pa + PHYS_VIRT_OFFSET
}
pub fn virt_to_phys(va: usize) -> usize {
    va - PHYS_VIRT_OFFSET
}
