#![no_std]
#![no_main]
extern crate alloc;

// lazy_static 会用到这个特性
mod driver;
mod bsp;
mod lang_items;
mod console;
mod mm;
mod system;
mod task;
mod interrupts;

use alloc::vec::Vec;
// use lang_items::*;
// 使用 core::arch::global_asm! 宏来包含整个汇编文件
use core::arch::{asm, global_asm};
use core::ptr::write_volatile;
use core::slice;
use driver::{SerialPort, Uart}; // 引入 Trait 和统一的 Uart 类型
use lazy_static::lazy_static;
use spin::Mutex;
use crate::bsp::qemu_virt::{get_mtimecmp_addr, QemuVirt, MTIME_ADDR, RISCV_ACLINT_DEFAULT_TIMEBASE_FREQ};
use crate::console::_print;
use crate::interrupts::{enable_machine_interrupts, init_mtimecmp};
use crate::mm::{init_heap, LockedAllocator};
use crate::system::SystemControl;

// 这行代码会把 entry.S 的内容直接嵌入到编译流程中
global_asm!(include_str!("entry.S"));

unsafe extern "C" {
    static _bss_start: usize;
    static _bss_end: usize;
}
unsafe fn clear_bss() {
        // 这段代码会清空 BSS 段
        let bss_start = unsafe { &_bss_start as *const _ as usize };
        let bss_end = unsafe { &_bss_end as *const _ as usize };
        println!("BSS start: {:x?}", bss_start);
        println!("BSS end: {:x?}", bss_end);
    let bss_size = bss_end - bss_start;
    unsafe {
        // 从裸指针和长度，创建一个可变的切片
        let bss_slice = slice::from_raw_parts_mut(bss_start as *mut u8, bss_size);
        // 调用切片的 fill 方法，高效地将整个区域清零
        bss_slice.fill(0);
    }
}
// 使用 lazy_static! 来创建我们唯一的、带锁的 UART 实例。
// 这是整个系统中对物理串口硬件的唯一表示。
lazy_static! {
    static ref UART: Mutex<Uart> = {
        // 这段代码只会在第一次访问 UART 时执行一次
        let mut uart = Uart::new();
        // 在创建的同时就完成初始化
        uart.init();
        Mutex::new(uart)
    };
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_main() {
    // 清空 BSS 段
    unsafe { clear_bss() };
    unsafe {
        init_mtimecmp();
        enable_machine_interrupts();
    }
    println!("Initializing heap...");
    init_heap();
    println!("Heap initialized.");
    let vec = Vec::from([1, 2, 3]);
    println!("vec 0: {}", vec[0]);
    println!("Hello from Charlotte OS!");

    let platform = QemuVirt;
    platform.shutdown();
}
