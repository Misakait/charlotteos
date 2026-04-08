// src/mm/mod.rs
pub mod address;
pub mod buddy;
pub mod bump;
pub mod memblock;
pub mod mm_set;
pub mod pagetable;
pub mod slub;

use crate::config::PHYS_VIRT_OFFSET;
use crate::data_struct::sync_ref_cell::SyncRefCell;
use crate::mm::address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use crate::mm::buddy::{phys_to_virt, virt_to_phys};
use crate::mm::bump::BumpAllocator;
use crate::mm::memblock::MEMBLOCK;
use crate::mm::pagetable::{FrameTracker, PTEFlags, PageSize, PageTable};
use crate::mm::slub::KmemCache;
use crate::{polling_println, println, sbi_println, virt_rust_main};
use buddy::BuddySystemFrameAllocator;
use core::alloc::{GlobalAlloc, Layout};
use core::arch::asm;
use core::cell::RefCell;
use core::num::NonZeroUsize;
use core::ptr::NonNull;
use core::ptr::{self, write_bytes};
use fdt::Fdt;
use lazy_static::lazy_static;
use riscv::register::satp::{self, Mode, Satp};
use spin::Mutex;
use spin::mutex::SpinMutex;

pub const PAGE_SIZE_BITS: usize = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_SIZE_BITS; // 4KB

unsafe extern "C" {
    static _skernel: usize;
    static _ekernel: usize;
    static _heap_size: usize;
    static _text_start: usize;
    static _text_end: usize;
    static _rodata_start: usize;
    static _rodata_end: usize;
    static _data_start: usize;
    static _data_end: usize;
    static _bss_start_with_stack: usize;
    static _bss_start: usize;
    static _bss_end: usize;
}
pub enum Slab {
    Slub {
        freelist: VirtAddr,
        inuse: u16,
        cache: NonNull<KmemCache>,
        has_next: bool,
        next_partial: PhysPageNum,
    },
    SlubTail {
        head_page_ppn: PhysPageNum,
    },
}
pub enum PageState {
    Reserved,
    Slab(Slab),
    Free,
    PageTable,
}

pub struct Page {
    pub ref_count: u8,
    pub state: PageState,
}
pub static mut MEM_MAP: &mut [Page] = &mut [];
pub static mut RAM_START_PPN: usize = 0;
pub static mut RAM_END_PPN: usize = 0;
pub static mut MEM_MAP_PA: PhysAddr = PhysAddr(0);

pub fn get_page_state(ppn: PhysPageNum) -> &'static mut Page {
    unsafe {
        let idx = ppn.0 - RAM_START_PPN;
        &mut MEM_MAP[idx]
    }
}
static BOOT_ROOT_PPN: SyncRefCell<PhysPageNum> = unsafe { SyncRefCell::new(PhysPageNum(0)) };

pub fn setup_memory_and_mapping(dtb_addr: usize) {
    // 这些链接脚本提供的也是物理上的
    let stext = unsafe { &_text_start as *const _ as usize };
    let etext = unsafe { &_text_end as *const _ as usize };
    let srodata = unsafe { &_rodata_start as *const _ as usize };
    let erodata = unsafe { &_rodata_end as *const _ as usize };
    let sdata = unsafe { &_data_start as *const _ as usize };
    let edata = unsafe { &_data_end as *const _ as usize };
    let sbss_with_stack = unsafe { &_bss_start_with_stack as *const _ as usize };
    let ebss = unsafe { &_bss_end as *const _ as usize };
    let skernel = unsafe { &_skernel as *const _ as usize };
    let ekernel = unsafe { &_ekernel as *const _ as usize };

    let fdt = unsafe { Fdt::from_ptr(dtb_addr as *const u8).unwrap() };

    // 获取物理内存总盘 (RAM)，并初始化 MEMBLOCK
    let mut ram_base = 0;
    let mut ram_size = 0;
    for node in fdt.all_nodes() {
        if node.name.starts_with("memory") {
            if let Some(memory_region) = node.reg().and_then(|mut reg| reg.next()) {
                ram_base = memory_region.starting_address as usize;
                ram_size = memory_region.size.unwrap_or(0);
                break;
            }
        }
    }
    let ram_end = ram_base + ram_size;
    MEMBLOCK
        .lock()
        .init_add_memory(PhysAddr(ram_base), ram_size);
    // polling_println!("Memblock init: raw_ram -> {:#x}..{:#x}", ram_base, ram_end);

    // 在 Memblock 中把内核占用的物理内存抠掉
    MEMBLOCK
        .lock()
        .reserve_memory(PhysAddr(skernel), ekernel - skernel);
    // polling_println!("Memblock reserve: kernel -> {:#x}..{:#x}", skernel, ekernel);

    // 处理头部旧标准的保留内存声明
    for reserved in fdt.memory_reservations() {
        MEMBLOCK
            .lock()
            .reserve_memory(PhysAddr(reserved.address() as usize), reserved.size());
        // polling_println!(
        //     "Memblock reserve: reserve_memory -> {:#x}..{:#x}",
        //     reserved.address() as usize,
        //     reserved.size() + reserved.size()
        // );
    }

    // 在 Memblock 抠除 DTB 数据本身的占用
    MEMBLOCK
        .lock()
        .reserve_memory(PhysAddr(dtb_addr), fdt.total_size());
    // polling_println!(
    //     "Memblock reserve: dtb -> {:#x}..{:#x}",
    //     dtb_addr,
    //     fdt.total_size() + dtb_addr
    // );

    // 申请根页表 (此时 Memblock 已经有内存了，可以安心申请)
    let root_pa = MEMBLOCK
        .lock()
        .early_alloc(PAGE_SIZE, PAGE_SIZE)
        .expect("boot root page table allocation failed");
    unsafe {
        write_bytes(root_pa.0 as *mut u8, 0, PAGE_SIZE);
    }
    let root_pt = unsafe { &mut *(root_pa.0 as *mut PageTable) };

    for node in fdt.all_nodes() {
        if node.name.starts_with("memory") {
            continue; // 真正的内存节点前面处理过了
        }

        if let Some(reg) = node.reg() {
            for memory_region in reg {
                let base = memory_region.starting_address as usize;
                let size = memory_region.size.unwrap_or(0);

                if size > 0 {
                    let end = base + size;

                    if base >= ram_base && end <= ram_end {
                        // 落在 RAM 里的区间,这是固件保留区 (比如 SBI)
                        MEMBLOCK.lock().reserve_memory(PhysAddr(base), size);
                        // polling_println!(
                        //     "Memblock reserve: reserve_memory -> {:#x}..{:#x}",
                        //     base,
                        //     base + size
                        // );
                    } else {
                        map_segment(
                            base,
                            end,
                            root_pt,
                            BootMapType::Linear,
                            MapAction::Map(PTEFlags::R | PTEFlags::W),
                        );
                        // polling_println!("Mapped MMIO: {} -> {:#x}..{:#x}", node.name, base, end);
                    }
                }
            }
        }
    }

    map_segment(
        stext,
        etext,
        root_pt,
        BootMapType::Linear,
        MapAction::Map(PTEFlags::R | PTEFlags::X),
    );
    map_segment(
        stext,
        etext,
        root_pt,
        BootMapType::Identical,
        MapAction::Map(PTEFlags::R | PTEFlags::X),
    );
    // polling_println!("Mapped text -> {:#x}..{:#x}", stext, etext);
    map_segment(
        srodata,
        erodata,
        root_pt,
        BootMapType::Linear,
        MapAction::Map(PTEFlags::R),
    );
    map_segment(
        srodata,
        erodata,
        root_pt,
        BootMapType::Identical,
        MapAction::Map(PTEFlags::R),
    );
    // polling_println!("Mapped rodata -> {:#x}..{:#x}", srodata, erodata);
    map_segment(
        sdata,
        edata,
        root_pt,
        BootMapType::Linear,
        MapAction::Map(PTEFlags::R | PTEFlags::W),
    );
    map_segment(
        sdata,
        edata,
        root_pt,
        BootMapType::Identical,
        MapAction::Map(PTEFlags::R | PTEFlags::W),
    );
    // polling_println!("Mapped data -> {:#x}..{:#x}", sdata, edata);
    map_segment(
        sbss_with_stack,
        ebss,
        root_pt,
        BootMapType::Linear,
        MapAction::Map(PTEFlags::R | PTEFlags::W),
    );
    map_segment(
        sbss_with_stack,
        ebss,
        root_pt,
        BootMapType::Identical,
        MapAction::Map(PTEFlags::R | PTEFlags::W),
    );
    // polling_println!("Mapped bss -> {:#x}..{:#x}", sbss_with_stack, ebss);

    // 内核终点到物理内存终点
    let ram_start_after_kernel = align_up(ekernel, PAGE_SIZE);
    map_segment(
        ram_start_after_kernel,
        ram_end,
        root_pt,
        BootMapType::Linear,
        MapAction::Map(PTEFlags::R | PTEFlags::W),
    );
    // polling_println!(
    //     "Mapped kernel -> {:#x}..{:#x}",
    //     ram_start_after_kernel,
    //     ram_end
    // );

    map_segment(
        ram_base,
        skernel,
        root_pt,
        BootMapType::Linear,
        MapAction::Map(PTEFlags::R | PTEFlags::W),
    );
    // polling_println!(
    //     "Mapped RAM before kernel -> {:#x}..{:#x}",
    //     ram_base,
    //     skernel
    // );

    // 设置内核根页表的PPN
    *BOOT_ROOT_PPN.borrow_mut() = PhysPageNum::from(root_pa);

    let ram_start_ppn = ram_base >> PAGE_SIZE_BITS;
    let ram_end_ppn = ram_end >> PAGE_SIZE_BITS;
    let total_pages = ram_end_ppn - ram_start_ppn;

    let mem_map_size = total_pages * core::mem::size_of::<Page>();
    let mem_map_pages = (mem_map_size + PAGE_SIZE - 1) >> PAGE_SIZE_BITS;

    // 向 Memblock 申请最后一块物理内存，存放 mem_map 自身
    let mem_map_pa = MEMBLOCK
        .lock()
        .early_alloc(mem_map_pages * PAGE_SIZE, PAGE_SIZE)
        .expect("Failed to allocate physical memory for mem_map array");

    // polling_println!(
    //     "Allocated mem_map array: {} pages at PA {:#x}",
    //     mem_map_pages,
    //     mem_map_pa.0
    // );

    unsafe {
        RAM_START_PPN = ram_start_ppn;
        RAM_END_PPN = ram_end_ppn;
        MEM_MAP_PA = mem_map_pa;
    }
}
enum BootMapType {
    Identical,
    Linear,
}
#[derive(Clone, Copy)]
pub enum MapAction {
    Map(PTEFlags),
    Unmap,
}
pub fn map_segment(
    start_addr: usize,
    end_addr: usize,
    root_pt: &mut PageTable,
    map_type: BootMapType,
    action: MapAction,
) {
    let page_start = align_down(start_addr, PAGE_SIZE);
    let page_end = align_up(end_addr, PAGE_SIZE);
    let aligned_start = align_up(page_start, 0x20_0000).min(page_end);
    let aligned_end = align_down(page_end, 0x20_0000).max(aligned_start);

    let pa_to_va = match map_type {
        BootMapType::Identical => |pa: usize| pa,
        BootMapType::Linear => |pa: usize| phys_to_virt(pa),
    };

    let mut apply_action = |pa: usize, size: PageSize| {
        let va = pa_to_va(pa);
        let vpn = VirtPageNum::from(VirtAddr::from(va));

        match action {
            MapAction::Map(flags) => {
                root_pt.bump_map(vpn, PhysPageNum::from(PhysAddr(pa)), flags, size);
            }
            MapAction::Unmap => {
                root_pt.unmap(vpn, size);
            }
        }
    };

    let head_end = aligned_start.min(page_end);
    let mut current_pa = page_start;
    while current_pa < head_end {
        apply_action(current_pa, PageSize::FourKB);
        current_pa += PAGE_SIZE;
    }

    let mut current_pa = aligned_start;
    let mid_end = aligned_end.min(page_end);
    while current_pa < mid_end {
        apply_action(current_pa, PageSize::TwoMB);
        current_pa += 0x20_0000;
    }

    let mut current_pa = mid_end;
    while current_pa < page_end {
        apply_action(current_pa, PageSize::FourKB);
        current_pa += PAGE_SIZE;
    }
}

fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

fn align_down(value: usize, align: usize) -> usize {
    value & !(align - 1)
}

type LockedBuddyAllocator = SpinMutex<BuddySystemFrameAllocator>;
pub static BUDDY_ALLOCATOR: LockedBuddyAllocator = SpinMutex::new(BuddySystemFrameAllocator::new());

pub fn init_buddy_system() {
    let ram_start = unsafe { RAM_START_PPN };
    let ram_end = unsafe { RAM_END_PPN };
    // sbi_println!("ram_start_ppn:{:#X},ram_end_ppn{:#X}", ram_start, ram_end);
    let total_pages = ram_end - ram_start;
    let mem_map_pa = unsafe { MEM_MAP_PA };
    let mem_map_va = phys_to_virt(mem_map_pa.0);
    unsafe {
        MEM_MAP = core::slice::from_raw_parts_mut(mem_map_va as *mut Page, total_pages);
    }
    for i in 0..total_pages {
        unsafe {
            MEM_MAP[i] = Page {
                ref_count: 1,
                state: PageState::Reserved,
            };
        }
    }

    let mb = MEMBLOCK.lock();
    let mut buddy = BUDDY_ALLOCATOR.lock();
    let mut free_pages_count: usize = 0;

    sbi_println!("Buddy System Allocator initialing:");
    // 遍历 Memblock 中剩余的 available
    for i in 0..mb.available.count {
        let region = &mb.available.regions[i];

        // 计算这块内存的起始和结束页号
        let start_addr = PhysAddr::from(region.base.0);
        let end_addr = PhysAddr::from(region.base.0 + region.size);
        let start_ppn = start_addr.ceil().0; // 向上取整，丢弃开头不完整的半页
        let end_ppn = end_addr.floor().0;

        // 在mem_map里，把这批页面的状态改写为 Free并加入Buddy System
        for ppn_idx in start_ppn..end_ppn {
            let local_idx = ppn_idx - ram_start;
            unsafe {
                MEM_MAP[local_idx] = Page {
                    ref_count: 0,
                    state: PageState::Free,
                };
            }
            free_pages_count += 1;
        }
        unsafe {
            buddy.add_free_region(start_ppn, end_ppn);
        }
    }
    // sbi_println!(
    //     "Handover Complete: {} total pages mapped. {} pages handed to Buddy.",
    //     total_pages,
    //     free_pages_count
    // );
}

pub fn enable_virtual_memory() {
    let root_ppn = BOOT_ROOT_PPN.borrow().0;

    let mut satp = Satp::from_bits(0);
    satp.set_mode(Mode::Sv39);
    satp.set_asid(0);
    satp.set_ppn(root_ppn);
    unsafe {
        satp::write(satp);
        asm!("sfence.vma");
    }
    // 函数地址依旧是物理实际上的
    let next_fn_virt_addr = phys_to_virt(virt_rust_main as fn() as *const () as usize);
    unsafe {
        core::arch::asm!(
            "add sp, sp, {offset}",
            "add s0, s0, {offset}",
            "jr {target}",
            offset = in(reg) PHYS_VIRT_OFFSET,
            target = in(reg) next_fn_virt_addr,
            options(noreturn)
        );
    }
}

pub fn unmap_temp_identity_area() {
    // let stext = unsafe { &_text_start as *const _ as usize };
    // let etext = unsafe { &_text_end as *const _ as usize };
    // let srodata = unsafe { &_rodata_start as *const _ as usize };
    // let erodata = unsafe { &_rodata_end as *const _ as usize };
    // let sdata = unsafe { &_data_start as *const _ as usize };
    // let edata = unsafe { &_data_end as *const _ as usize };
    // let sbss_with_stack = unsafe { &_bss_start_with_stack as *const _ as usize };
    // let ebss = unsafe { &_bss_end as *const _ as usize };
    let stext = virt_to_phys(unsafe { &_text_start as *const _ as usize });
    let etext = virt_to_phys(unsafe { &_text_end as *const _ as usize });
    let srodata = virt_to_phys(unsafe { &_rodata_start as *const _ as usize });
    let erodata = virt_to_phys(unsafe { &_rodata_end as *const _ as usize });
    let sdata = virt_to_phys(unsafe { &_data_start as *const _ as usize });
    let edata = virt_to_phys(unsafe { &_data_end as *const _ as usize });
    let sbss_with_stack = virt_to_phys(unsafe { &_bss_start_with_stack as *const _ as usize });
    let ebss = virt_to_phys(unsafe { &_bss_end as *const _ as usize });

    let ppn: PhysAddr = (&*BOOT_ROOT_PPN.borrow()).into();
    let root_pa = ppn;
    let root_pt_va = phys_to_virt(root_pa.0);
    let root_pt = unsafe { &mut *(root_pt_va as *mut PageTable) };

    map_segment(
        stext,
        etext,
        root_pt,
        BootMapType::Identical,
        MapAction::Unmap,
    );
    map_segment(
        srodata,
        erodata,
        root_pt,
        BootMapType::Identical,
        MapAction::Unmap,
    );

    map_segment(
        sdata,
        edata,
        root_pt,
        BootMapType::Identical,
        MapAction::Unmap,
    );
    map_segment(
        sbss_with_stack,
        ebss,
        root_pt,
        BootMapType::Identical,
        MapAction::Unmap,
    );
}
