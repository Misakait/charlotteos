// src/mm/mod.rs
pub mod address;
pub mod buddy;
pub mod bump;
pub mod memblock;
pub mod mm_set;
pub mod pagetable;

use crate::config::PHYS_VIRT_OFFSET;
use crate::data_struct::sync_ref_cell::SyncRefCell;
use crate::mm::address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use crate::mm::buddy::phys_to_virt;
use crate::mm::bump::BumpAllocator;
use crate::mm::memblock::MEMBLOCK;
use crate::mm::pagetable::{FrameTracker, PTEFlags, PageSize, PageTable};
use crate::{polling_println, println};
use buddy::BuddySystemFrameAllocator;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::RefCell;
use core::num::NonZeroUsize;
use core::ptr;
use core::ptr::NonNull;
use fdt::Fdt;
use lazy_static::lazy_static;
use spin::Mutex;
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
    static _memory_end: usize;
    static _memory_start: usize;
}

static BOOT_ROOT_PPN: SyncRefCell<PhysPageNum> = unsafe { SyncRefCell::new(PhysPageNum(0)) };

pub fn setup_memory_and_mapping(dtb_addr: usize) {
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
    polling_println!("Memblock init: raw_ram -> {:#x}..{:#x}", ram_base, ram_end);

    // 在 Memblock 中把内核占用的物理内存抠掉
    MEMBLOCK
        .lock()
        .reserve_memory(PhysAddr(skernel), ekernel - skernel);
    polling_println!("Memblock reserve: kernel -> {:#x}..{:#x}", skernel, ekernel);

    // 处理头部旧标准的保留内存声明
    for reserved in fdt.memory_reservations() {
        MEMBLOCK
            .lock()
            .reserve_memory(PhysAddr(reserved.address() as usize), reserved.size());
        polling_println!(
            "Memblock reserve: reserve_memory -> {:#x}..{:#x}",
            reserved.address() as usize,
            reserved.size() + reserved.size()
        );
    }

    // 在 Memblock 抠除 DTB 数据本身的占用
    MEMBLOCK
        .lock()
        .reserve_memory(PhysAddr(dtb_addr), fdt.total_size());
    polling_println!(
        "Memblock reserve: dtb -> {:#x}..{:#x}",
        dtb_addr,
        fdt.total_size() + dtb_addr
    );

    // 申请根页表 (此时 Memblock 已经有内存了，可以安心申请)
    let root_pa = MEMBLOCK
        .lock()
        .early_alloc(PAGE_SIZE, PAGE_SIZE)
        .expect("boot root page table allocation failed");
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
                        polling_println!(
                            "Memblock reserve: reserve_memory -> {:#x}..{:#x}",
                            base,
                            base + size
                        );
                    } else {
                        map_segment(base, end, PTEFlags::R | PTEFlags::W, root_pt);
                        polling_println!("Mapped MMIO: {} -> {:#x}..{:#x}", node.name, base, end);
                    }
                }
            }
        }
    }

    // 内核精细化映射
    map_segment(stext, etext, PTEFlags::R | PTEFlags::X, root_pt);
    polling_println!("Mapped text -> {:#x}..{:#x}", stext, etext);
    map_segment(srodata, erodata, PTEFlags::R, root_pt);
    polling_println!("Mapped rodata -> {:#x}..{:#x}", srodata, erodata);
    map_segment(sdata, edata, PTEFlags::R | PTEFlags::W, root_pt);
    polling_println!("Mapped data -> {:#x}..{:#x}", sdata, edata);
    map_segment(sbss_with_stack, ebss, PTEFlags::R | PTEFlags::W, root_pt);
    polling_println!("Mapped bss -> {:#x}..{:#x}", sbss_with_stack, ebss);

    // 内核终点到物理内存终点
    let ram_start_after_kernel = align_up(ekernel, PAGE_SIZE);
    map_segment(
        ram_start_after_kernel,
        ram_end,
        PTEFlags::R | PTEFlags::W,
        root_pt,
    );
    polling_println!(
        "Mapped kernel -> {:#x}..{:#x}",
        ram_start_after_kernel,
        ram_end
    );

    // 6. 将根页表移交给激活阶段
    *BOOT_ROOT_PPN.borrow_mut() = PhysPageNum::from(root_pa);
}

pub fn map_segment(start_addr: usize, end_addr: usize, flags: PTEFlags, root_pt: &mut PageTable) {
    let page_start = align_down(start_addr, PAGE_SIZE);
    let page_end = align_up(end_addr, PAGE_SIZE);
    let aligned_start = align_up(page_start, 0x20_0000).min(page_end);
    let aligned_end = align_down(page_end, 0x20_0000).max(aligned_start);

    let head_end = aligned_start.min(page_end);
    let mut current_pa = page_start;
    while current_pa < head_end {
        let va = current_pa + PHYS_VIRT_OFFSET;
        let vpn = VirtAddr::from(va);
        root_pt.bump_map(
            VirtPageNum::from(vpn),
            PhysPageNum::from(PhysAddr(current_pa)),
            flags,
            PageSize::FourKB,
        );
        current_pa += PAGE_SIZE;
    }

    let mut current_pa = aligned_start;
    let mid_end = aligned_end.min(page_end);
    while current_pa < mid_end {
        let va = current_pa + PHYS_VIRT_OFFSET;
        let vpn = VirtAddr::from(va);
        root_pt.bump_map(
            VirtPageNum::from(vpn),
            PhysPageNum::from(PhysAddr(current_pa)),
            flags,
            PageSize::TwoMB,
        );
        current_pa += 0x20_0000;
    }

    let mut current_pa = mid_end;
    while current_pa < page_end {
        let va = current_pa + PHYS_VIRT_OFFSET;
        let vpn = VirtAddr::from(va);
        root_pt.bump_map(
            VirtPageNum::from(vpn),
            PhysPageNum::from(PhysAddr(current_pa)),
            flags,
            PageSize::FourKB,
        );
        current_pa += PAGE_SIZE;
    }
}

fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

fn align_down(value: usize, align: usize) -> usize {
    value & !(align - 1)
}

fn init_buddy_system() {}
