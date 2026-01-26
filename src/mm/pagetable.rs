use core::{mem::forget, num::NonZeroUsize, slice::from_mut};

use alloc::vec::Vec;
use bitflags::bitflags;

use crate::mm::{
    BUMP_ALLOCATOR, FRAME_ALLOCATOR,
    address::{PPN_WIDTH_SV39, PhysAddr, PhysPageNum, VirtPageNum},
    buddy::phys_to_virt,
    frame_alloc, frame_dealloc,
};

bitflags! {
    #[derive(Copy, Clone)]
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
pub struct FrameTracker {
    pub ppn: PhysPageNum,
}
impl FrameTracker {
    pub fn new(ppn: PhysPageNum) -> Self {
        ppn.clear();
        Self { ppn }
    }
}
impl Drop for FrameTracker {
    fn drop(&mut self) {
        frame_dealloc(self.ppn);
    }
}
pub enum PageSize {
    FourKB,
    TwoMB,
    OneGB,
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

    pub fn ppn(&self) -> PhysPageNum {
        PhysPageNum((self.bits >> 10) & ((1 << PPN_WIDTH_SV39) - 1))
    }

    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits_truncate(self.bits as u8)
    }

    const fn empty() -> PageTableEntry {
        PageTableEntry { bits: 0 }
    }
}

#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; 512],
}

impl PageTable {
    pub const fn new() -> Self {
        PageTable {
            entries: [PageTableEntry::empty(); 512],
        }
    }
    pub fn bump_find_pte(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indices();
        let mut entries = &mut self.entries;
        for i in 0..3 {
            let pte = &mut entries[idxs[i]];
            if i == 2 {
                return Some(pte);
            }
            if !pte.is_valid() {
                return None;
            }
            let phys_addr = PhysAddr::from(&pte.ppn()).0;
            // let virt_addr = phys_to_virt(phys_addr);
            unsafe {
                entries = &mut *(phys_addr as *mut [PageTableEntry; 512]);
                // entries = &mut *(virt_addr as *mut [PageTableEntry; 512]);
            }
        }
        None
    }
    pub fn bump_find_create_pte(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indices();
        let mut entries = &mut self.entries;
        for i in 0..3 {
            let pte = &mut entries[idxs[i]];
            if i == 2 {
                return Some(pte);
            }
            if !pte.is_valid() {
                let ppn = BUMP_ALLOCATOR.borrow_mut().alloc_page().unwrap();
                *pte = PageTableEntry::new(ppn, PTEFlags::V);
            }
            let phys_addr = PhysAddr::from(&pte.ppn()).0;
            // let virt_addr = phys_to_virt(phys_addr);
            unsafe {
                // entries = &mut *(virt_addr as *mut [PageTableEntry; 512]);
                entries = &mut *(phys_addr as *mut [PageTableEntry; 512]);
            }
        }
        None
    }
    pub fn bump_find_create_pte_2mb(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indices();
        let mut entries = &mut self.entries;
        for i in 0..2 {
            let pte = &mut entries[idxs[i]];
            if i == 1 {
                return Some(pte);
            }
            if !pte.is_valid() {
                let ppn = BUMP_ALLOCATOR.borrow_mut().alloc_page().unwrap();
                *pte = PageTableEntry::new(ppn, PTEFlags::V);
            }
            let phys_addr = PhysAddr::from(&pte.ppn()).0;
            // let virt_addr = phys_to_virt(phys_addr);
            unsafe {
                // entries = &mut *(virt_addr as *mut [PageTableEntry; 512]);
                entries = &mut *(phys_addr as *mut [PageTableEntry; 512]);
            }
        }
        None
    }
    pub fn find_pte(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indices();
        let mut entries = &mut self.entries;
        for i in 0..3 {
            let pte = &mut entries[idxs[i]];
            if i == 2 {
                return Some(pte);
            }
            if !pte.is_valid() {
                return None;
            }
            let phys_addr = PhysAddr::from(&pte.ppn()).0;
            let virt_addr = phys_to_virt(phys_addr);
            unsafe {
                entries = &mut *(virt_addr as *mut [PageTableEntry; 512]);
            }
        }
        None
    }
    pub fn find_create_pte(
        &mut self,
        vpn: VirtPageNum,
        frames: &mut Vec<FrameTracker>,
    ) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indices();
        let mut entries = &mut self.entries;
        for i in 0..3 {
            let pte = &mut entries[idxs[i]];
            if i == 2 {
                return Some(pte);
            }
            if !pte.is_valid() {
                let frame = frame_alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                frames.push(frame);
            }
            let phys_addr = PhysAddr::from(&pte.ppn()).0;
            let virt_addr = phys_to_virt(phys_addr);
            unsafe {
                entries = &mut *(virt_addr as *mut [PageTableEntry; 512]);
            }
        }
        None
    }
    pub fn index(&mut self, index: usize) -> &mut PageTableEntry {
        &mut self.entries[index]
    }
    pub fn bump_map(
        &mut self,
        vpn: VirtPageNum,
        ppn: PhysPageNum,
        flags: PTEFlags,
        size: PageSize,
    ) {
        let pte = match size {
            PageSize::FourKB => self.bump_find_create_pte(vpn).unwrap(),
            PageSize::TwoMB => self.bump_find_create_pte_2mb(vpn).unwrap(),
            PageSize::OneGB => unimplemented!(),
        };

        assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }
    pub fn map(
        &mut self,
        vpn: VirtPageNum,
        ppn: PhysPageNum,
        flags: PTEFlags,
        frames: &mut Vec<FrameTracker>,
    ) {
        let pte = self.find_create_pte(vpn, frames).unwrap();

        assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }
    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.bump_find_pte(vpn).unwrap();
        assert!(
            pte.is_valid(),
            "vpn {:?} is must be valid before unmapping",
            vpn
        );
        *pte = PageTableEntry::empty();
    }
}
