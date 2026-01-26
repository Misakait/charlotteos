use core::ptr;

use crate::mm::{
    PAGE_SIZE,
    address::{PhysAddr, PhysPageNum},
};

pub struct BumpAllocator {
    start: PhysPageNum,
    pub current: PhysPageNum,
    end: PhysPageNum,
}
impl BumpAllocator {
    pub fn new(start: usize, end: usize) -> Self {
        let start_addr = PhysAddr(start);
        let end_addr = PhysAddr(end);
        Self {
            start: start_addr.ceil(),
            current: start_addr.ceil(),
            end: end_addr.floor(),
        }
    }
    pub fn alloc_page(&mut self) -> Option<PhysPageNum> {
        if self.current < self.end {
            let current = self.current;
            let pa = PhysAddr::from(&current).0;
            unsafe {
                ptr::write_bytes(pa as *mut u8, 0, PAGE_SIZE);
            }
            self.current.0 += 1;
            Some(current)
        } else {
            None
        }
    }
}
