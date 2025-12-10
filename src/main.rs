#![no_std]
#![no_main]
extern crate alloc;

mod bsp;
mod console;
mod data_struct;
mod driver;
mod lang_items;
mod mm;
mod system;
mod task;
mod trap;

use alloc::vec::Vec;
// use lang_items::*;
use crate::bsp::get_hart_id;
use crate::bsp::qemu_virt::{
    IER, MTIME_ADDR, QemuVirt, RISCV_ACLINT_DEFAULT_TIMEBASE_FREQ, UART_BASE, UART0_IRQ,
    get_mtimecmp_addr, plic_context_addr, plic_enable_addr, plic_priority_addr,
};
use crate::console::_print;
use crate::data_struct::ring_buf::RingBuffer;
use crate::mm::{LockedAllocator, init_heap};
use crate::system::SystemControl;
use crate::task::SCHEDULER;
use crate::task::context::TaskContext;
use crate::task::scheduler::{Scheduler, trampoline};
use crate::trap::interrupts::{init_machine_interrupts, set_mtimecmp};
use crate::trap::{trap_entry, trap_handler};
use core::arch::{asm, global_asm, naked_asm};
use core::ptr::{read_volatile, write_volatile};
use core::slice;
use core::sync::atomic::{AtomicUsize, Ordering};
use driver::{SerialPort, Uart}; // 引入 Trait 和统一的 Uart 类型
use lazy_static::lazy_static;
use spin::Mutex;

// 这行代码会把 entry.S 的内容直接嵌入到编译流程中
global_asm!(include_str!("entry.S"));

unsafe extern "C" {
    static _bss_start: usize;
    static _bss_end: usize;
}
fn clear_bss() {
    // 这段代码会清空 BSS 段
    let bss_start = unsafe { &_bss_start as *const _ as usize };
    let bss_end = unsafe { &_bss_end as *const _ as usize };
    //     println!("BSS start: {:x?}", bss_start);
    //     println!("BSS end: {:x?}", bss_end);
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
        let mut uart = Uart::new(UART_BASE);
        // 在创建的同时就完成初始化
        uart.init();
        Mutex::new(uart)
    };
}
static mut KERNEL_INIT_CONTEXT: TaskContext = TaskContext::zero();
#[unsafe(no_mangle)]
pub extern "C" fn rust_main() {
    // 清空 BSS 段
    clear_bss();

    // println!("Initializing heap...");
    init_heap();
    // println!("Heap initialized.");

    unsafe {
        asm!("csrw mscratch, {}", in(reg) &raw mut KERNEL_INIT_CONTEXT);
        let mtvec_addr = (trap_entry as usize) & !0x3;
        asm!("csrw mtvec, {}", in(reg) mtvec_addr);
    }
    let vec = Vec::from([1, 2, 3]);
    // println!("vec ptr: {:#X}", vec.as_ptr() as *const usize as usize);
    // polling_println!("polling");

    // 初始化调度器并创建 idle 任务
    let _ = Scheduler::init();
    // println!("✓ Scheduler initialized with idle task");

    // 创建测试任务
    {
        let mut scheduler = SCHEDULER.lock();
        scheduler
            .spawn(test_task_a, 8192, 1)
            .expect("Failed to spawn task A");
        scheduler
            .spawn(test_task_b, 8192, 1)
            .expect("Failed to spawn task B");
    } // 锁在这里释放

    // println!("All tasks created. Starting scheduler...");
    // println!("======================================================");

    unsafe {
        set_mtimecmp();
        init_machine_interrupts();
    }
    println!("tram: {:X}", trampoline as usize);
    println!("Hello from Charlotte OS!");
    Scheduler::run_scheduler();

    // loop {}
    let platform = QemuVirt;
    platform.shutdown();
}
#[unsafe(no_mangle)]
pub extern "C" fn test_task_a() {
    println!("[Task A] ✓ Start!");
    let mut a = 0;
    for _ in 0..10 {
        // 模拟一些工作负载
        for _ in 0..100000 {
            // println!("[Task A] num is:{} ", a);
            a += 1;
            core::hint::spin_loop();
        }
    }
    println!("[Task A] ✓ Finished!");
}
extern "C" fn test_task_b() {
    println!("[Task B] ✓ Start!");
    let mut b = 0;
    for _ in 0..10 {
        // 模拟一些工作负载
        for _ in 0..100000 {
            b += 1;
            core::hint::spin_loop();
        }
    }
    println!("[Task B] ✓ Finished!");
}
