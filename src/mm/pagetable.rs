use bitflags::bitflags;

use crate::mm::address::{PPN_WIDTH_SV39, PhysPageNum};
bitflags! {
    pub struct PTEFlags: u8 {
        const V = 1 << 0; // Valid: 该项是否有效
        const R = 1 << 1; // Read
        const W = 1 << 2; // Write
        const X = 1 << 3; // Execute
        const U = 1 << 4; // User: 用户态是否可访问
        const G = 1 << 5; // Global: 全局映射（通常用于内核共享部分）
        const A = 1 << 6; // Accessed: 硬件自动设置，表示被访问过
        const D = 1 << 7; // Dirty: 硬件自动设置，表示被写入过
    }
}

#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct PageTableEntry {
    pub bits: usize,
}

impl PageTableEntry {
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        PageTableEntry {
            // PPN 在 PTE 中是从第 10 位开始的
            bits: ((ppn.0) << 10) | flags.bits() as usize,
        }
    }

    pub fn is_valid(&self) -> bool {
        (self.bits & PTEFlags::V.bits() as usize) != 0
    }

    pub fn ppn(&self) -> usize {
        (self.bits >> 10) & ((1 << PPN_WIDTH_SV39) - 1)
    }

    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits_truncate(self.bits as u8)
    }
}

#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; 512],
}

impl PageTable {
    pub fn map_at(&mut self, index: usize) -> &mut PageTableEntry {
        &mut self.entries[index]
    }
}
