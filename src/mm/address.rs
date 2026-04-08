use crate::mm::buddy::phys_to_virt;
use crate::mm::{PAGE_SIZE, PAGE_SIZE_BITS};

use core::convert::From;
use core::{ptr, usize};

pub const PA_WIDTH_SV39: usize = 56;
pub const VA_WIDTH_SV39: usize = 39;
pub const PPN_WIDTH_SV39: usize = PA_WIDTH_SV39 - PAGE_SIZE_BITS;
pub const VPN_WIDTH_SV39: usize = VA_WIDTH_SV39 - PAGE_SIZE_BITS;
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct PhysAddr(pub usize);

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(transparent)]
pub struct VirtAddr(pub usize);

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct PhysPageNum(pub usize);

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct VirtPageNum(pub usize);
trait CanNext {
    fn next(&mut self);
}
impl VirtPageNum {
    pub fn indices(&self) -> [usize; 3] {
        let mut vpn = self.0;
        let mut idx = [0; 3];
        for i in (0..3).rev() {
            idx[i] = vpn & ((1 << 9) - 1);
            vpn >>= 9;
        }
        idx
    }
}
impl CanNext for VirtPageNum {
    fn next(&mut self) {
        self.0 = self.0 + 1;
    }
}

impl VirtAddr {
    pub fn floor(&self) -> VirtPageNum {
        VirtPageNum(self.0 >> PAGE_SIZE_BITS)
    }

    pub fn ceil(&self) -> VirtPageNum {
        VirtPageNum((self.0 - 1 + PAGE_SIZE) >> PAGE_SIZE_BITS)
    }

    pub fn page_offset(&self) -> usize {
        self.0 & (PAGE_SIZE - 1)
    }

    /// Check if the virtual address is aligned by page size
    pub fn aligned(&self) -> bool {
        self.page_offset() == 0
    }
}
impl From<VirtAddr> for VirtPageNum {
    fn from(v: VirtAddr) -> Self {
        assert_eq!(v.page_offset(), 0);
        v.floor()
    }
}
impl From<VirtPageNum> for VirtAddr {
    fn from(v: VirtPageNum) -> Self {
        Self::from(v.0 << PAGE_SIZE_BITS)
    }
}
impl From<usize> for VirtAddr {
    fn from(v: usize) -> Self {
        // 25=64-39
        let valid_va = ((v << 25) as isize >> 25) as usize;
        Self(valid_va)
    }
}
impl From<usize> for VirtPageNum {
    fn from(v: usize) -> Self {
        Self(v & ((1 << VPN_WIDTH_SV39) - 1))
    }
}
impl From<usize> for PhysAddr {
    fn from(v: usize) -> Self {
        Self(v & ((1 << PA_WIDTH_SV39) - 1))
    }
}

impl From<usize> for PhysPageNum {
    fn from(v: usize) -> Self {
        Self(v & ((1 << PPN_WIDTH_SV39) - 1))
    }
}

impl From<PhysAddr> for usize {
    fn from(v: PhysAddr) -> Self {
        v.0
    }
}

impl From<PhysPageNum> for usize {
    fn from(v: PhysPageNum) -> Self {
        v.0
    }
}

impl PhysAddr {
    pub fn page_offset(&self) -> usize {
        self.0 & (PAGE_SIZE - 1)
    }
    pub fn floor(&self) -> PhysPageNum {
        PhysPageNum(self.0 >> PAGE_SIZE_BITS)
    }
    pub fn ceil(&self) -> PhysPageNum {
        PhysPageNum((self.0 + PAGE_SIZE - 1) >> PAGE_SIZE_BITS)
    }
}

impl From<PhysAddr> for PhysPageNum {
    fn from(pa: PhysAddr) -> Self {
        assert_eq!(pa.page_offset(), 0);

        pa.floor()
    }
}

impl From<&PhysPageNum> for PhysAddr {
    fn from(v: &PhysPageNum) -> Self {
        Self(v.0 << PAGE_SIZE_BITS)
    }
}
impl PhysPageNum {
    pub fn clear(&self) {
        let pa = PhysAddr::from(self);
        unsafe {
            let va = phys_to_virt(pa.0);
            ptr::write_bytes(va as *mut u8, 0, PAGE_SIZE);
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Range<T>
where
    T: CanNext + Copy + Clone + Ord + PartialOrd + Eq + PartialEq,
{
    current: T,
    end: T,
}

impl<T> Range<T>
where
    T: CanNext + Copy + Clone + Ord + PartialOrd + Eq + PartialEq,
{
    /// 创建一个左闭右开的区间 [start, end)
    pub fn new(start: T, end: T) -> Self {
        Self {
            current: start,
            end,
        }
    }
}

impl<T> Iterator for Range<T>
where
    T: CanNext + Copy + Clone + Ord + PartialOrd + Eq + PartialEq,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            let val = self.current;
            self.current.next();
            Some(val)
        } else {
            None
        }
    }
}

pub type VPNRange = Range<VirtPageNum>;
