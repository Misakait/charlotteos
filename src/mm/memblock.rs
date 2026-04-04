use core::cmp::{max, min};

use spin::{Spin, mutex::SpinMutex};

use crate::mm::address::PhysAddr;

const MAX_REGIONS: usize = 128;

#[derive(Clone, Copy, Debug)]
pub struct MemRegion {
    pub base: PhysAddr,
    pub size: usize,
}

impl MemRegion {
    pub fn end(&self) -> PhysAddr {
        PhysAddr(self.base.0 + self.size)
    }
}

pub struct RegionArray {
    pub regions: [MemRegion; MAX_REGIONS],
    pub count: usize,
    pub total_size: usize,
}

impl RegionArray {
    pub const fn new() -> Self {
        Self {
            regions: [MemRegion {
                base: PhysAddr(0),
                size: 0,
            }; MAX_REGIONS],
            count: 0,
            total_size: 0,
        }
    }

    fn update_total(&mut self) {
        self.total_size = self.regions[0..self.count].iter().map(|r| r.size).sum();
    }

    pub fn add_region(&mut self, base: PhysAddr, size: usize) {
        if size == 0 {
            return;
        }
        let end = PhysAddr(base.0 + size);

        let mut insert_idx = 0;
        while insert_idx < self.count && self.regions[insert_idx].end() < base {
            insert_idx += 1;
        }

        let mut i = insert_idx;
        let mut merged_base = base;
        let mut merged_end = end;

        while i < self.count && self.regions[i].base <= merged_end {
            merged_base = PhysAddr(min(merged_base.0, self.regions[i].base.0));
            merged_end = PhysAddr(max(merged_end.0, self.regions[i].end().0));
            i += 1;
        }

        let merge_count = i - insert_idx;
        if merge_count == 0 {
            assert!(self.count < MAX_REGIONS, "Region array full!");
            self.regions
                .copy_within(insert_idx..self.count, insert_idx + 1);
            self.count += 1;
        } else {
            let shift = merge_count - 1;
            if shift > 0 {
                self.regions.copy_within(i..self.count, i - shift);
                self.count -= shift;
            }
        }

        self.regions[insert_idx] = MemRegion {
            base: merged_base,
            size: merged_end.0 - merged_base.0,
        };
        self.update_total();
    }

    /// 求差集：从现有数组中抠除一段内存
    pub fn remove_region(&mut self, rm_base: PhysAddr, rm_size: usize) {
        if rm_size == 0 {
            return;
        }
        let rm_end = PhysAddr(rm_base.0 + rm_size);

        let mut i = 0;
        while i < self.count {
            let reg_base = self.regions[i].base;
            let reg_end = self.regions[i].end();

            // 检查是否有交集
            if rm_base < reg_end && rm_end > reg_base {
                if rm_base <= reg_base && rm_end >= reg_end {
                    // 精准命中或全覆盖，直接删除该节点
                    self.regions.copy_within(i + 1..self.count, i);
                    self.count -= 1;
                    continue; // 删除后，下一个元素补到了位置 i，所以 i 不自增
                } else if rm_base <= reg_base && rm_end < reg_end {
                    // 切除头部
                    self.regions[i].base = rm_end;
                    self.regions[i].size = reg_end.0 - rm_end.0;
                } else if rm_base > reg_base && rm_end >= reg_end {
                    // 切除尾部
                    self.regions[i].size = rm_base.0 - reg_base.0;
                } else {
                    // 中间打洞，一分为二
                    assert!(
                        self.count < MAX_REGIONS,
                        "Region array full during hole punching!"
                    );

                    // 将原本的区域缩短，作为左半部分
                    self.regions[i].size = rm_base.0 - reg_base.0;

                    // 把 i 后面的元素全部往后挪一位，腾出位置给右半部分
                    self.regions.copy_within(i + 1..self.count, i + 2);

                    // 插入右半部分
                    self.regions[i + 1] = MemRegion {
                        base: rm_end,
                        size: reg_end.0 - rm_end.0,
                    };
                    self.count += 1;

                    i += 1; // 跳过新插入的右半部分，避免重复检查
                }
            }
            i += 1;
        }
        self.update_total();
    }
}
pub struct Memblock {
    pub available: RegionArray,
    pub reserved: RegionArray,
    pub allocated: RegionArray,
}

pub static MEMBLOCK: SpinMutex<Memblock> = SpinMutex::new(Memblock {
    available: RegionArray::new(),
    reserved: RegionArray::new(),
    allocated: RegionArray::new(),
});

impl Memblock {
    /// 初始化：声明系统总内存
    pub fn init_add_memory(&mut self, base: PhysAddr, size: usize) {
        self.available.add_region(base, size);
    }

    /// 打洞：保留固件区域 (从 available 抠除，加入 reserved)
    pub fn reserve_memory(&mut self, base: PhysAddr, size: usize) {
        self.available.remove_region(base, size);
        self.reserved.add_region(base, size);
    }

    pub fn early_alloc(&mut self, size: usize, align: usize) -> Option<PhysAddr> {
        for i in (0..self.available.count).rev() {
            let reg = &self.available.regions[i];

            // 计算向下对齐的起始物理地址
            let alloc_base = (reg.end().0 - size) & !(align - 1);

            if alloc_base >= reg.base.0 {
                let phys_addr = PhysAddr(alloc_base);
                self.available.remove_region(phys_addr, size);
                self.allocated.add_region(phys_addr, size);
                return Some(phys_addr);
            }
        }
        None
    }
}
