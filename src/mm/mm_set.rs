use alloc::{collections::btree_map::BTreeMap, vec::Vec};
use bitflags::bitflags;

use crate::mm::pagetable::PageTable;
bitflags! {
    pub struct MapPermission: u8 {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum MapType {
    Identical,
    Framed,
}
// pub struct MapArea {
//     vpn_range: VPNRange,
//     data_frames: BTreeMap<VirtPageNum, FrameTracker>,
//     map_type: MapType,
//     map_perm: MapPermission,
// }
// pub struct MemorySet {
//     page_table: PageTable,
//     areas: Vec<MapArea>,
// }
