use bitflags::Flag;
use core::{
    alloc::GlobalAlloc,
    alloc::Layout,
    num::NonZeroUsize,
    ptr::{write_unaligned, write_volatile},
};
use spin::mutex::SpinMutex;
use thiserror_no_std::Error;

use crate::{
    data_struct::lock::IrqLock,
    mm::{
        BUDDY_ALLOCATOR, MEM_MAP, PAGE_SIZE, PAGE_SIZE_BITS, PageState, Slab,
        address::{PhysAddr, PhysPageNum, VirtAddr},
        buddy::{phys_to_virt, virt_to_phys},
        get_page_state,
    },
};
#[derive(Error, Debug)]
pub enum KernelError {
    #[error("Slub: out of memory")]
    OutOfMemory,
}
pub struct KmemCache {
    pub object_size: usize,
    pub active_slab_ppn: Option<PhysPageNum>,
    pub partial_slabs_head: Option<PhysPageNum>,
    // pub partial_count: usize,
    // pub min_partial: usize,
}
pub struct SlubAllocator {
    // 管理从2^3=8字节到2^11=2048字节，所以数组长度是11-3+1=9
    caches: [IrqLock<KmemCache>; 9],
}

unsafe impl GlobalAlloc for SlubAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        let required_size = size.max(align).next_power_of_two();
        // 最小 8 字节
        let actual_size = required_size.max(8);

        if actual_size <= 2048 {
            // 路由到对应的 KmemCache
            let index = actual_size.trailing_zeros() as usize - 3;
            return self.caches[index].lock().alloc();
        } else {
            // 大于 2048B 的，找 Buddy 批发大页
            unimplemented!()
            // return buddy_alloc_large(actual_size);
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let required_size = layout.size().max(layout.align()).next_power_of_two();
        let actual_size = required_size.max(8);

        if actual_size > 2048 {
            // 大页模式直接还给 Buddy
            let va = ptr as usize;
            let pa = virt_to_phys(va);
            let ppn = PhysPageNum::from(pa);
            let pages = actual_size >> PAGE_SIZE_BITS;

            for i in 0..pages {
                let current_ppn = PhysPageNum(ppn.0 + i);
                let page = get_page_state(current_ppn);
                page.state = PageState::Free;
                page.ref_count = 0;
            }

            BUDDY_ALLOCATOR
                .lock()
                .dealloc(ppn, NonZeroUsize::new(pages).unwrap());
        } else {
            // SLUB 模式
            let va = ptr as usize;
            let pa = virt_to_phys(va);
            let ppn = PhysPageNum::from(pa);

            let page = get_page_state(ppn);

            match &mut page.state {
                PageState::Slab(Slab::Slub { cache, .. }) => unsafe {
                    cache.as_mut().free_object(ptr, ppn);
                },
                PageState::Slab(Slab::SlubTail { head_page_ppn }) => {
                    let head_page = get_page_state(*head_page_ppn);
                    if let PageState::Slab(Slab::Slub { cache, .. }) = &mut head_page.state {
                        unsafe {
                            cache.as_mut().free_object(ptr, *head_page_ppn);
                        }
                    } else {
                        panic!("FATAL: SlubTail does not point to a Slub Head!");
                    }
                }
                _ => {
                    panic!("FATAL: Tried to dealloc a pointer not belonging to SLUB!");
                }
            }
        }
    }
}
#[global_allocator]
static SLUB_ALLOCATOR: SlubAllocator = SlubAllocator {
    caches: [
        IrqLock::new(KmemCache::new(8)),
        IrqLock::new(KmemCache::new(16)),
        IrqLock::new(KmemCache::new(32)),
        IrqLock::new(KmemCache::new(64)),
        IrqLock::new(KmemCache::new(128)),
        IrqLock::new(KmemCache::new(256)),
        IrqLock::new(KmemCache::new(512)),
        IrqLock::new(KmemCache::new(1024)),
        IrqLock::new(KmemCache::new(2048)),
    ],
};

impl KmemCache {
    pub const fn new(requested_size: usize) -> Self {
        // 获取当前机器硬件字长
        let align = core::mem::size_of::<usize>();
        let min_size = align;

        // 向上取整到字长的倍数
        let mut actual_size = (requested_size + align - 1) & !(align - 1);
        if actual_size < min_size {
            actual_size = min_size;
        }

        Self {
            object_size: actual_size,
            active_slab_ppn: None,
            partial_slabs_head: None,
        }
    }

    fn alloc_new_slab(&mut self) -> Result<PhysPageNum, KernelError> {
        if self.object_size < PAGE_SIZE {
            let ppn = BUDDY_ALLOCATOR
                .lock()
                .alloc(NonZeroUsize::new(1).unwrap())
                .ok_or(KernelError::OutOfMemory)?;
            // 将slab结构分割成对应大小的node节点
            let start_va = phys_to_virt(PhysAddr::from(&ppn).0);
            let mut va = start_va;
            let end = va + PAGE_SIZE;

            while va < end {
                let memory_block = va as *mut usize;

                let next_va = va + self.object_size;
                let next_addr = if next_va < end { next_va } else { 0 };

                unsafe {
                    write_volatile(memory_block, next_addr);
                }
                va += self.object_size;
            }

            let original_head = self.partial_slabs_head;
            let (has_next, next_partial) = if let Some(head_ppn) = original_head {
                (true, head_ppn)
            } else {
                (false, PhysPageNum(0))
            };
            let mut slab = Slab::Slub {
                freelist: VirtAddr::from(start_va),
                inuse: 0,
                cache: self.into(),
                has_next,
                next_partial,
            };
            // 头插法插入
            self.partial_slabs_head = Some(ppn);
            // 更改新获得的内存对应的MEM_MAP的信息为slab
            let page = get_page_state(ppn);
            page.ref_count = 1;
            page.state = PageState::Slab(slab);

            return Ok(ppn);
        } else {
            // 大页分配逻辑
            let required_pages = (self.object_size + PAGE_SIZE - 1) >> PAGE_SIZE_BITS;

            // Buddy System 只能分配 2 的次幂个页，向上取整到 2 的幂
            let allocate_pages = required_pages.next_power_of_two();

            let ppn = BUDDY_ALLOCATOR
                .lock()
                .alloc(core::num::NonZeroUsize::new(allocate_pages).unwrap())
                .ok_or(KernelError::OutOfMemory)?;

            let start_va = phys_to_virt(PhysAddr::from(&ppn).0);
            let mut va = start_va;
            let end = start_va + (allocate_pages << PAGE_SIZE_BITS);

            while va + self.object_size <= end {
                let memory_block = va as *mut usize;
                let next_va = va + self.object_size;

                let next_addr = if next_va + self.object_size <= end {
                    next_va
                } else {
                    0
                };

                unsafe {
                    write_volatile(memory_block, next_addr);
                }
                va += self.object_size;
            }

            let original_head = self.partial_slabs_head;
            let (has_next, next_partial) = if let Some(head_ppn) = original_head {
                (true, head_ppn)
            } else {
                (false, PhysPageNum(0))
            };

            let slab_head = Slab::Slub {
                freelist: VirtAddr::from(start_va),
                inuse: 0,
                cache: self.into(),
                has_next,
                next_partial,
            };

            self.partial_slabs_head = Some(ppn);

            let current_ppn = PhysPageNum(ppn.0);
            let page = get_page_state(current_ppn);
            page.ref_count = 1;
            page.state = PageState::Slab(slab_head);
            // 后面的页标记为Tail
            for i in 1..allocate_pages {
                let current_ppn = PhysPageNum(ppn.0 + i);
                let page = get_page_state(current_ppn);
                page.ref_count = 1;

                page.state = PageState::Slab(Slab::SlubTail {
                    head_page_ppn: ppn, // 全部指回头Slab
                });
            }

            return Ok(ppn);
        }
    }
    fn alloc(&mut self) -> *mut u8 {
        loop {
            // 尝试从当前正在使用的 Active Slab 中分配
            if let Some(ppn) = self.active_slab_ppn {
                let page = get_page_state(ppn);

                if let PageState::Slab(Slab::Slub {
                    freelist, inuse, ..
                }) = &mut page.state
                {
                    // 检查 Active Slab 是否已满，
                    // 由let next_addr = if next_va < end { next_va } else { 0 };这一行保证
                    if freelist.0 == 0 {
                        self.active_slab_ppn = None;
                        continue;
                    }

                    // 拿走当前 freelist 指向的内存块
                    let mem_block_ptr = freelist.0 as *mut u8;

                    // 读取这个内存块开头里存的下一个空闲内存块的地址
                    let next_free_addr = unsafe { *(mem_block_ptr as *mut usize) };

                    *freelist = VirtAddr(next_free_addr);
                    *inuse += 1;

                    return mem_block_ptr;
                } else {
                    panic!("FATAL: active_slab_ppn does not point to a Slub Head!");
                }
            }
            // Active 没有可用内存，从 Partial 获取
            else if let Some(partial_head) = self.partial_slabs_head.take() {
                let page = get_page_state(partial_head);

                if let PageState::Slab(Slab::Slub {
                    has_next,
                    next_partial,
                    ..
                }) = &page.state
                {
                    if *has_next {
                        self.partial_slabs_head = Some(*next_partial);
                    }
                    self.active_slab_ppn = Some(partial_head);
                    continue;
                } else {
                    panic!("FATAL: Memory corruption! Found non-Head page in partial_slabs_head");
                }
            }
            // 若partial也没有可用内存，向buddy system申请新的
            else {
                match self.alloc_new_slab() {
                    Ok(new_ppn) => {
                        self.active_slab_ppn = Some(new_ppn);
                        continue;
                    }
                    Err(_) => {
                        // OOM
                        return core::ptr::null_mut();
                    }
                }
            }
        }
    }

    pub unsafe fn free_object(&mut self, ptr: *mut u8, slab_ppn: PhysPageNum) {
        let page = get_page_state(slab_ppn);

        if let PageState::Slab(Slab::Slub {
            freelist,
            inuse,
            has_next,
            next_partial,
            ..
        }) = &mut page.state
        {
            let was_full = freelist.0 == 0;
            let is_active = self.active_slab_ppn == Some(slab_ppn);

            let obj_ptr = ptr as *mut usize;
            // 将当前的 freelist (下一个可用地址) 写入被释放对象的开头
            unsafe {
                write_volatile(obj_ptr, freelist.0);
            }
            // freelist 更新为当前刚释放的对象
            *freelist = VirtAddr::from(ptr as usize);
            *inuse -= 1;

            if *inuse == 0 {
                if is_active {
                    self.active_slab_ppn = None;
                } else {
                    self.remove_from_partial(slab_ppn);
                }

                // 当初向 Buddy 申请的页数
                let required_pages = (self.object_size + PAGE_SIZE - 1) >> PAGE_SIZE_BITS;
                let allocate_pages = required_pages.max(1).next_power_of_two();

                for i in 0..allocate_pages {
                    let current_ppn = PhysPageNum(slab_ppn.0 + i);
                    let current_page = get_page_state(current_ppn);
                    current_page.state = PageState::Free;
                    current_page.ref_count = 0;
                }

                BUDDY_ALLOCATOR.lock().dealloc(
                    slab_ppn,
                    core::num::NonZeroUsize::new(allocate_pages).unwrap(),
                );
            } else if was_full && !is_active {
                // 曾经全满,没有保存在kmemcacahe中
                // 如果它本身就是 active，哪怕曾满了也不用管，因为下一次 alloc 会处理它。
                // 只有当它既满了，又不是 active 时，才说明它被遗忘了。
                // 现在它空出了一个位置，把它头插法放回 Partial 备用链表

                let old_head = self.partial_slabs_head;
                if let Some(old_ppn) = old_head {
                    *next_partial = old_ppn;
                    *has_next = true;
                } else {
                    *has_next = false;
                }
                self.partial_slabs_head = Some(slab_ppn);
            }
        } else {
            panic!(
                "FATAL: free_object called on a non-Slub page! PPN: {:?}",
                slab_ppn
            );
        }
    }
    fn remove_from_partial(&mut self, target_ppn: PhysPageNum) {
        let mut current_opt = self.partial_slabs_head;
        let mut prev_opt: Option<PhysPageNum> = None;

        while let Some(current_ppn) = current_opt {
            let current_page = get_page_state(current_ppn);

            // 提取当前节点的 next 指针
            let next_opt = if let PageState::Slab(Slab::Slub {
                has_next,
                next_partial,
                ..
            }) = &current_page.state
            {
                if *has_next { Some(*next_partial) } else { None }
            } else {
                panic!("FATAL: Non-Slub page found in partial_slabs_head!");
            };

            // 命中目标，开始物理摘除
            if current_ppn == target_ppn {
                if let Some(prev_ppn) = prev_opt {
                    // 目标在链表中间或尾部。让上一个节点直接指向下一个节点
                    let prev_page = get_page_state(prev_ppn);
                    if let PageState::Slab(Slab::Slub {
                        has_next,
                        next_partial,
                        ..
                    }) = &mut prev_page.state
                    {
                        if let Some(n) = next_opt {
                            *next_partial = n;
                            *has_next = true;
                        } else {
                            *has_next = false;
                        }
                    }
                } else {
                    // 目标就是头节点。直接修改 KmemCache 的头指针
                    self.partial_slabs_head = next_opt;
                }
                return;
            }

            // 没找到，双指针继续向后推进
            prev_opt = Some(current_ppn);
            current_opt = next_opt;
        }

        panic!(
            "FATAL: Tried to remove PPN {:?} but it was not in the partial list!",
            target_ppn
        );
    }
}
